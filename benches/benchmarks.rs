use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use criterion::{Criterion, criterion_group, criterion_main};
use daimon::api::{AppState, router};
use daimon::edge::EdgeFleetManager;
use daimon::mcp::McpHostRegistry;
use daimon::rag::{RagConfig, RagPipeline};
use daimon::scheduler::TaskScheduler;
use daimon::supervisor::Supervisor;
use daimon::vector_store::{VectorIndex, cosine_similarity};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn test_state() -> Arc<AppState> {
    Arc::new(AppState {
        config: daimon::Config::default(),
        mcp: RwLock::new(McpHostRegistry::new()),
        mcp_handlers: HashMap::new(),
        rag: RwLock::new(RagPipeline::new(rag_config(50, 10))),
        edge: RwLock::new(EdgeFleetManager::default()),
        scheduler: RwLock::new(TaskScheduler::new()),
        supervisor: RwLock::new(Supervisor::default()),
    })
}

/// Build a VectorEntry via serde (workaround for #[non_exhaustive]).
fn make_entry(dim: usize, seed: usize) -> daimon::vector_store::VectorEntry {
    let embedding: Vec<f64> = (0..dim).map(|j| ((seed * dim + j) as f64).sin()).collect();
    serde_json::from_value(json!({
        "id": uuid::Uuid::new_v4(),
        "embedding": embedding,
        "metadata": null,
        "content": format!("doc-{seed}"),
        "created_at": chrono::Utc::now(),
    }))
    .unwrap()
}

/// Build a RagConfig via serde.
fn rag_config(chunk_size: usize, overlap: usize) -> RagConfig {
    serde_json::from_value(json!({
        "top_k": 5,
        "chunk_size": chunk_size,
        "overlap": overlap,
        "min_relevance_score": 0.1,
        "context_template": "Use the following context to answer the question.\n\n---\n{context}\n---\n\nQuestion: {query}",
    }))
    .unwrap()
}

/// Build an McpToolDescription via serde.
fn make_tool(name: &str) -> daimon::mcp::McpToolDescription {
    serde_json::from_value(json!({
        "name": name,
        "description": format!("Tool: {name}"),
        "inputSchema": {"type": "object"},
    }))
    .unwrap()
}

// ---------------------------------------------------------------------------
// Core benchmarks
// ---------------------------------------------------------------------------

fn bench_config_default(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(daimon::Config::default);
    });
}

fn bench_cosine_similarity(c: &mut Criterion) {
    let a: Vec<f64> = (0..128).map(|i| (i as f64).sin()).collect();
    let b: Vec<f64> = (0..128).map(|i| (i as f64).cos()).collect();
    c.bench_function("cosine_similarity_128d", |bench| {
        bench.iter(|| cosine_similarity(&a, &b));
    });
}

fn bench_vector_insert(c: &mut Criterion) {
    c.bench_function("vector_insert_128d", |bench| {
        bench.iter(|| {
            let mut idx = VectorIndex::new();
            for i in 0..100 {
                idx.insert(make_entry(128, i)).unwrap();
            }
        });
    });
}

fn bench_vector_search(c: &mut Criterion) {
    let mut idx = VectorIndex::new();
    for i in 0..1000 {
        idx.insert(make_entry(64, i)).unwrap();
    }
    let query: Vec<f64> = (0..64).map(|i| (i as f64).cos()).collect();

    c.bench_function("vector_search_1k_64d_top10", |bench| {
        bench.iter(|| idx.search(&query, 10));
    });
}

fn bench_rag_ingest(c: &mut Criterion) {
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(100);

    c.bench_function("rag_ingest_5k_chars", |bench| {
        bench.iter(|| {
            let mut pipeline = RagPipeline::new(rag_config(50, 10));
            pipeline
                .ingest_text(&text, serde_json::Value::Null)
                .unwrap();
        });
    });
}

fn bench_scheduler_schedule_pending(c: &mut Criterion) {
    use daimon::scheduler::*;

    c.bench_function("scheduler_100_tasks_10_nodes", |bench| {
        bench.iter(|| {
            let mut sched = TaskScheduler::new();
            for i in 0..10 {
                sched.register_node(NodeCapacity::new(
                    format!("n{i}"),
                    8.0,
                    16384,
                    102400,
                    false,
                ));
            }
            for i in 0..100 {
                let task = ScheduledTask::new(
                    format!("task-{i}"),
                    "bench",
                    "agent-1",
                    ((i % 10) + 1) as u8,
                    ResourceReq::default(),
                );
                sched.submit_task(task).unwrap();
            }
            sched.schedule_pending()
        });
    });
}

// ---------------------------------------------------------------------------
// Agent registration throughput
// ---------------------------------------------------------------------------

fn bench_supervisor_register(c: &mut Criterion) {
    c.bench_function("supervisor_register_1000_agents", |bench| {
        bench.iter(|| {
            let mut sup = Supervisor::default();
            for _ in 0..1000 {
                sup.register_agent(agnostik::AgentId::new());
            }
        });
    });
}

// ---------------------------------------------------------------------------
// MCP dispatch benchmarks
// ---------------------------------------------------------------------------

fn bench_mcp_register_tools(c: &mut Criterion) {
    c.bench_function("mcp_register_100_tools", |bench| {
        bench.iter(|| {
            let mut reg = McpHostRegistry::new();
            for i in 0..100 {
                reg.register_builtin(make_tool(&format!("tool-{i}")));
            }
        });
    });
}

fn bench_mcp_manifest(c: &mut Criterion) {
    let mut reg = McpHostRegistry::new();
    for i in 0..100 {
        reg.register_builtin(make_tool(&format!("tool-{i:03}")));
    }

    c.bench_function("mcp_manifest_100_tools", |bench| {
        bench.iter(|| reg.manifest());
    });
}

fn bench_mcp_find_tool(c: &mut Criterion) {
    let mut reg = McpHostRegistry::new();
    for i in 0..100 {
        reg.register_builtin(make_tool(&format!("tool-{i:03}")));
    }

    c.bench_function("mcp_find_tool_in_100", |bench| {
        bench.iter(|| reg.find_tool("tool-050"));
    });
}

// ---------------------------------------------------------------------------
// API latency benchmarks (in-process, no network)
// ---------------------------------------------------------------------------

fn bench_api_health(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("api_health_latency", |bench| {
        bench.iter(|| {
            let state = test_state();
            rt.block_on(async {
                let app = router(state);
                app.oneshot(Request::get("/v1/health").body(Body::empty()).unwrap())
                    .await
                    .unwrap()
            })
        });
    });
}

fn bench_api_mcp_tools_list(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("api_mcp_tools_list_latency", |bench| {
        bench.iter(|| {
            let state = test_state();
            rt.block_on(async {
                let app = router(state);
                app.oneshot(Request::get("/v1/mcp/tools").body(Body::empty()).unwrap())
                    .await
                    .unwrap()
            })
        });
    });
}

fn bench_api_edge_stats(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("api_edge_stats_latency", |bench| {
        bench.iter(|| {
            let state = test_state();
            rt.block_on(async {
                let app = router(state);
                app.oneshot(Request::get("/v1/edge/stats").body(Body::empty()).unwrap())
                    .await
                    .unwrap()
            })
        });
    });
}

fn bench_api_metrics(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("api_metrics_latency", |bench| {
        bench.iter(|| {
            let state = test_state();
            rt.block_on(async {
                let app = router(state);
                app.oneshot(Request::get("/v1/metrics").body(Body::empty()).unwrap())
                    .await
                    .unwrap()
            })
        });
    });
}

// ---------------------------------------------------------------------------
// Edge fleet benchmarks
// ---------------------------------------------------------------------------

fn bench_edge_register_nodes(c: &mut Criterion) {
    use daimon::edge::*;

    c.bench_function("edge_register_100_nodes", |bench| {
        bench.iter(|| {
            let mut mgr = EdgeFleetManager::default();
            for i in 0..100 {
                mgr.register_node(
                    format!("node-{i}"),
                    EdgeCapabilities::default(),
                    "/usr/bin/agent".into(),
                    "0.1.0".into(),
                    "Linux".into(),
                    "http://localhost:8090".into(),
                )
                .unwrap();
            }
        });
    });
}

// ---------------------------------------------------------------------------
// RAG query benchmarks
// ---------------------------------------------------------------------------

fn bench_rag_query(c: &mut Criterion) {
    let mut pipeline = RagPipeline::new(rag_config(100, 20));
    for i in 0..50 {
        pipeline
            .ingest_text(
                &format!("Document {i} contains information about topic {}", i % 10),
                json!({"doc": i}),
            )
            .unwrap();
    }

    c.bench_function("rag_query_50_docs", |bench| {
        bench.iter(|| pipeline.query_text("information about topic 5"));
    });
}

// ---------------------------------------------------------------------------
// Federation scoring benchmarks
// ---------------------------------------------------------------------------

fn bench_federation_scoring(c: &mut Criterion) {
    use daimon::federation::*;

    // Build nodes and requirements via serde (workaround for #[non_exhaustive]).
    let reqs: AgentRequirements = serde_json::from_value(json!({
        "cpu_cores": 2,
        "memory_mb": 4096,
        "gpu_required": false,
        "preferred_node": null,
        "affinity_nodes": [],
    }))
    .unwrap();

    let nodes: Vec<FederationNode> = (0..100)
        .map(|i| {
            serde_json::from_value(json!({
                "node_id": format!("node-{i}"),
                "name": format!("Node {i}"),
                "address": "127.0.0.1:8080",
                "role": "Follower",
                "status": "Online",
                "last_heartbeat": chrono::Utc::now(),
                "capabilities": {
                    "cpu_cores": 8,
                    "memory_mb": 16384,
                    "gpu_count": if i % 3 == 0 { 1 } else { 0 },
                },
                "current_term": 1,
                "voted_for": null,
            }))
            .unwrap()
        })
        .collect();

    c.bench_function("federation_score_100_nodes", |bench| {
        bench.iter(|| {
            let scorer = NodeScorer::new();
            for node in &nodes {
                let _ = scorer.score_node(node, &reqs);
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Edge fleet heartbeat throughput
// ---------------------------------------------------------------------------

fn bench_edge_heartbeat(c: &mut Criterion) {
    use daimon::edge::*;

    let mut mgr = EdgeFleetManager::default();
    let ids: Vec<String> = (0..100)
        .map(|i| {
            mgr.register_node(
                format!("hb-node-{i}"),
                EdgeCapabilities::default(),
                "/usr/bin/agent".into(),
                "0.1.0".into(),
                "Linux".into(),
                "http://localhost:8090".into(),
            )
            .unwrap()
        })
        .collect();

    let hb: HeartbeatData = serde_json::from_value(json!({
        "active_tasks": 2,
        "tasks_completed": 50,
    }))
    .unwrap();

    c.bench_function("edge_heartbeat_100_nodes", |bench| {
        bench.iter(|| {
            for id in &ids {
                mgr.heartbeat(id, hb.clone()).unwrap();
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Edge fleet stats aggregation
// ---------------------------------------------------------------------------

fn bench_edge_stats(c: &mut Criterion) {
    use daimon::edge::*;

    let mut mgr = EdgeFleetManager::default();
    for i in 0..500 {
        mgr.register_node(
            format!("stat-node-{i}"),
            EdgeCapabilities::default(),
            "/usr/bin/agent".into(),
            "0.1.0".into(),
            "Linux".into(),
            "http://localhost:8090".into(),
        )
        .unwrap();
    }

    c.bench_function("edge_stats_500_nodes", |bench| {
        bench.iter(|| mgr.stats());
    });
}

criterion_group!(
    benches,
    // Core
    bench_config_default,
    bench_cosine_similarity,
    bench_vector_insert,
    bench_vector_search,
    bench_rag_ingest,
    bench_rag_query,
    bench_scheduler_schedule_pending,
    // Agent registration throughput
    bench_supervisor_register,
    // MCP dispatch
    bench_mcp_register_tools,
    bench_mcp_manifest,
    bench_mcp_find_tool,
    // API latency
    bench_api_health,
    bench_api_mcp_tools_list,
    bench_api_edge_stats,
    bench_api_metrics,
    // Edge fleet
    bench_edge_register_nodes,
    bench_edge_heartbeat,
    bench_edge_stats,
    // Federation
    bench_federation_scoring,
);
criterion_main!(benches);
