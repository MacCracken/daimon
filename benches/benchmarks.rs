use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use criterion::{Criterion, criterion_group, criterion_main};
use daimon::api::{AppState, router};
use daimon::edge::EdgeFleetManager;
use daimon::mcp::McpToolRegistry;
use daimon::rag::{RagConfig, RagPipeline};
use daimon::scheduler::TaskScheduler;
use daimon::supervisor::Supervisor;
use daimon::vector_store::{VectorIndex, cosine_similarity};
use serde_json::json;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn test_state() -> Arc<AppState> {
    Arc::new(AppState {
        config: daimon::Config::default(),
        mcp: RwLock::new(McpToolRegistry::new()),
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
        "input_schema": {"type": "object"},
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
            let mut reg = McpToolRegistry::new();
            for i in 0..100 {
                reg.register_builtin(make_tool(&format!("tool-{i}")));
            }
        });
    });
}

fn bench_mcp_manifest(c: &mut Criterion) {
    let mut reg = McpToolRegistry::new();
    for i in 0..100 {
        reg.register_builtin(make_tool(&format!("tool-{i:03}")));
    }

    c.bench_function("mcp_manifest_100_tools", |bench| {
        bench.iter(|| reg.manifest());
    });
}

fn bench_mcp_find_tool(c: &mut Criterion) {
    let mut reg = McpToolRegistry::new();
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

criterion_group!(
    benches,
    // Core
    bench_config_default,
    bench_cosine_similarity,
    bench_vector_insert,
    bench_vector_search,
    bench_rag_ingest,
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
);
criterion_main!(benches);
