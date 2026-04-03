use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tokio::sync::RwLock;
use tower::ServiceExt;

use daimon::api::{AppState, router};
use daimon::edge::EdgeFleetManager;
use daimon::mcp::McpHostRegistry;
use daimon::rag::{RagConfig, RagPipeline};
use daimon::scheduler::TaskScheduler;
use daimon::supervisor::Supervisor;
use daimon::{Config, DaimonError};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_state() -> Arc<AppState> {
    #[cfg(feature = "http-forward")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    Arc::new(AppState {
        config: Config::default(),
        mcp: RwLock::new(McpHostRegistry::new()),
        mcp_handlers: std::collections::HashMap::new(),
        rag: RwLock::new(RagPipeline::new(RagConfig::default())),
        edge: RwLock::new(EdgeFleetManager::default()),
        scheduler: RwLock::new(TaskScheduler::new()),
        supervisor: RwLock::new(Supervisor::default()),
    })
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1_048_576)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn delete(uri: &str) -> Request<Body> {
    Request::delete(uri).body(Body::empty()).unwrap()
}

// ---------------------------------------------------------------------------
// Error type tests
// ---------------------------------------------------------------------------

#[test]
fn error_display_agent_not_found() {
    let err = DaimonError::AgentNotFound("test-agent".into());
    assert!(err.to_string().contains("test-agent"));
    assert!(err.to_string().contains("agent not found"));
}

#[test]
fn error_display_invalid_parameter() {
    let err = DaimonError::InvalidParameter("bad port".into());
    assert!(err.to_string().contains("bad port"));
}

#[test]
fn error_display_agent_already_exists() {
    let err = DaimonError::AgentAlreadyExists("dup-agent".into());
    assert!(err.to_string().contains("dup-agent"));
}

#[test]
fn error_display_supervisor() {
    let err = DaimonError::SupervisorError("process crashed".into());
    assert!(err.to_string().contains("process crashed"));
}

#[test]
fn error_display_ipc() {
    let err = DaimonError::IpcError("socket closed".into());
    assert!(err.to_string().contains("socket closed"));
}

#[test]
fn error_display_scheduler() {
    let err = DaimonError::SchedulerError("queue full".into());
    assert!(err.to_string().contains("queue full"));
}

#[test]
fn error_display_federation() {
    let err = DaimonError::FederationError("peer unreachable".into());
    assert!(err.to_string().contains("peer unreachable"));
}

#[test]
fn error_display_api() {
    let err = DaimonError::ApiError("bad request".into());
    assert!(err.to_string().contains("bad request"));
}

#[test]
fn error_display_storage() {
    let err = DaimonError::StorageError("disk full".into());
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
    let err = DaimonError::from(io_err);
    assert!(err.to_string().contains("missing file"));
}

// ---------------------------------------------------------------------------
// Config tests
// ---------------------------------------------------------------------------

#[test]
fn config_default_values() {
    let cfg = Config::default();
    assert_eq!(cfg.listen_addr, "127.0.0.1");
    assert_eq!(cfg.port, 8090);
    assert_eq!(cfg.data_dir, "/var/lib/agnos");
    assert_eq!(cfg.max_agents, 1000);
}

#[test]
fn config_serde_roundtrip() {
    let cfg = Config::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.port, cfg.port);
    assert_eq!(deserialized.listen_addr, cfg.listen_addr);
    assert_eq!(deserialized.data_dir, cfg.data_dir);
    assert_eq!(deserialized.max_agents, cfg.max_agents);
}

#[test]
fn config_custom_values() {
    let json = r#"{"listen_addr":"0.0.0.0","port":9090,"data_dir":"/tmp/daimon","max_agents":50}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.listen_addr, "0.0.0.0");
    assert_eq!(cfg.port, 9090);
    assert_eq!(cfg.data_dir, "/tmp/daimon");
    assert_eq!(cfg.max_agents, 50);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: Health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_ok() {
    let app = router(test_state());
    let resp = app.oneshot(get("/v1/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
    assert!(!json["version"].as_str().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// HTTP round-trip: MCP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mcp_register_list_deregister() {
    let state = test_state();

    // Register external tool
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/mcp/tools",
            json!({
                "name": "scan",
                "description": "port scanner",
                "input_schema": {"type": "object"},
                "callback_url": "http://localhost:9999/scan"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List tools
    let app = router(state.clone());
    let resp = app.oneshot(get("/v1/mcp/tools")).await.unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["tools"].as_array().unwrap().len(), 1);
    assert_eq!(json["tools"][0]["name"], "scan");

    // Call the tool — callback URL is unreachable in tests, so we get an error result.
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/mcp/call",
            json!({"name": "scan", "arguments": {}}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["isError"].as_bool().unwrap());

    // Deregister
    let app = router(state.clone());
    let resp = app.oneshot(delete("/v1/mcp/tools/scan")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify empty
    let app = router(state.clone());
    let resp = app.oneshot(get("/v1/mcp/tools")).await.unwrap();
    let json = body_json(resp).await;
    assert!(json["tools"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn mcp_call_unknown_tool() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/mcp/call",
            json!({"name": "nonexistent", "arguments": {}}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: RAG
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rag_ingest_and_query() {
    let state = test_state();

    // Ingest
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/rag/ingest",
            json!({
                "text": "Rust is a systems programming language focused on safety and performance.",
                "metadata": {"source": "test"}
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert!(!json["chunk_ids"].as_array().unwrap().is_empty());

    // Query
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json("/v1/rag/query", json!({"query": "rust safety"})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(
        json["formatted_context"]
            .as_str()
            .unwrap()
            .contains("rust safety")
    );
}

#[tokio::test]
async fn rag_ingest_empty_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/rag/ingest",
            json!({"text": "", "metadata": null}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rag_query_empty_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json("/v1/rag/query", json!({"query": ""})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: Edge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edge_register_heartbeat_decommission() {
    let state = test_state();

    // Register
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json("/v1/edge/nodes", json!({"name": "edge-1"})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    let node_id = json["id"].as_str().unwrap().to_string();

    // Get node
    let app = router(state.clone());
    let resp = app
        .oneshot(get(&format!("/v1/edge/nodes/{node_id}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["name"], "edge-1");
    assert_eq!(json["status"], "Online");

    // Heartbeat
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            &format!("/v1/edge/nodes/{node_id}/heartbeat"),
            json!({"active_tasks": 3, "tasks_completed": 10}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Stats
    let app = router(state.clone());
    let resp = app.oneshot(get("/v1/edge/stats")).await.unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["total_nodes"], 1);
    assert_eq!(json["online"], 1);

    // List with filter
    let app = router(state.clone());
    let resp = app
        .oneshot(get("/v1/edge/nodes?status=online"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["nodes"].as_array().unwrap().len(), 1);

    // Decommission
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            &format!("/v1/edge/nodes/{node_id}/decommission"),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify decommissioned
    let app = router(state.clone());
    let resp = app
        .oneshot(get(&format!("/v1/edge/nodes/{node_id}")))
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["status"], "Decommissioned");
}

#[tokio::test]
async fn edge_register_duplicate_rejected() {
    let state = test_state();

    let app = router(state.clone());
    app.oneshot(post_json("/v1/edge/nodes", json!({"name": "dup"})))
        .await
        .unwrap();

    let app = router(state.clone());
    let resp = app
        .oneshot(post_json("/v1/edge/nodes", json!({"name": "dup"})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: Scheduler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scheduler_submit_schedule_cancel() {
    let state = test_state();

    // Register a node
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/nodes",
            json!({
                "node_id": "worker-1",
                "total_cpu": 8.0,
                "total_memory_mb": 16384
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Submit a task
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/tasks",
            json!({
                "name": "train-model",
                "description": "fine-tune LLM",
                "agent_id": "agent-1",
                "priority": 7
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    let task_id = json["task_id"].as_str().unwrap().to_string();

    // Get task
    let app = router(state.clone());
    let resp = app
        .oneshot(get(&format!("/v1/scheduler/tasks/{task_id}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["name"], "train-model");
    assert_eq!(json["status"], "Queued");

    // Schedule pending
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json("/v1/scheduler/schedule", json!({})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["decisions"].as_array().unwrap().len(), 1);

    // Stats
    let app = router(state.clone());
    let resp = app.oneshot(get("/v1/scheduler/stats")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total_tasks"], 1);

    // Submit another and cancel it
    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/tasks",
            json!({
                "name": "cancel-me",
                "agent_id": "agent-2",
                "priority": 3
            }),
        ))
        .await
        .unwrap();
    let json = body_json(resp).await;
    let cancel_id = json["task_id"].as_str().unwrap().to_string();

    let app = router(state.clone());
    let resp = app
        .oneshot(post_json(
            &format!("/v1/scheduler/tasks/{cancel_id}/cancel"),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn scheduler_submit_empty_name_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/tasks",
            json!({"name": "", "agent_id": "a1"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: Metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_aggregates_subsystems() {
    let state = test_state();

    // Register an edge node to make metrics non-zero
    let app = router(state.clone());
    app.oneshot(post_json("/v1/edge/nodes", json!({"name": "m-node"})))
        .await
        .unwrap();

    let app = router(state.clone());
    let resp = app.oneshot(get("/v1/metrics")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["edge_nodes"], 1);
    assert_eq!(json["mcp_tools"], 0);
}

// ---------------------------------------------------------------------------
// HTTP round-trip: Agents (supervisor)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agents_list_empty() {
    let app = router(test_state());
    let resp = app.oneshot(get("/v1/agents")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["agents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn agent_not_found() {
    let app = router(test_state());
    let resp = app
        .oneshot(get("/v1/agents/00000000-0000-0000-0000-000000000001"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn agent_invalid_id() {
    let app = router(test_state());
    let resp = app.oneshot(get("/v1/agents/not-a-uuid")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// 404 for unknown routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = router(test_state());
    let resp = app.oneshot(get("/v1/nonexistent")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// -- Additional edge case tests --

#[tokio::test]
async fn register_scheduler_node_zero_cpu_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/nodes",
            json!({"node_id": "n1", "total_cpu": 0.0, "total_memory_mb": 1024}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_scheduler_node_zero_memory_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/scheduler/nodes",
            json!({"node_id": "n1", "total_cpu": 4.0, "total_memory_mb": 0}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_edge_node_empty_name_rejected() {
    let app = router(test_state());
    let resp = app
        .oneshot(post_json(
            "/v1/edge/nodes",
            json!({"name": "", "capabilities": {}}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
