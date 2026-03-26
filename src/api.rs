//! HTTP API server — axum-based REST API on port 8090.
//!
//! Endpoints: /v1/health, /v1/agents, /v1/metrics, /v1/mcp, /v1/rag, /v1/edge, /v1/scheduler.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;
use crate::edge::{EdgeCapabilities, EdgeFleetManager, EdgeNodeStatus, HeartbeatData};
use crate::error::{DaimonError, Result};
use crate::mcp::{McpToolCall, McpToolRegistry, McpToolResult, RegisterMcpToolRequest};
use crate::rag::{RagConfig, RagPipeline};
use crate::scheduler::{NodeCapacity, ResourceReq, ScheduledTask, TaskScheduler};
use crate::supervisor::Supervisor;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Shared state for the API server.
pub struct AppState {
    /// Service configuration.
    pub config: Config,
    /// MCP tool registry.
    pub mcp: RwLock<McpToolRegistry>,
    /// RAG pipeline.
    pub rag: RwLock<RagPipeline>,
    /// Edge fleet manager.
    pub edge: RwLock<EdgeFleetManager>,
    /// Task scheduler.
    pub scheduler: RwLock<TaskScheduler>,
    /// Supervisor.
    pub supervisor: RwLock<Supervisor>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the axum router with all API routes.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health
        .route("/v1/health", get(health))
        // Agents (supervisor)
        .route("/v1/agents", get(list_agents))
        .route("/v1/agents/:agent_id", get(get_agent))
        // MCP
        .route("/v1/mcp/tools", get(list_mcp_tools).post(register_mcp_tool))
        .route("/v1/mcp/tools/:name", delete(deregister_mcp_tool))
        .route("/v1/mcp/call", post(call_mcp_tool))
        // RAG
        .route("/v1/rag/ingest", post(rag_ingest))
        .route("/v1/rag/query", post(rag_query))
        // Edge
        .route(
            "/v1/edge/nodes",
            get(list_edge_nodes).post(register_edge_node),
        )
        .route("/v1/edge/nodes/:node_id", get(get_edge_node))
        .route("/v1/edge/nodes/:node_id/heartbeat", post(edge_heartbeat))
        .route(
            "/v1/edge/nodes/:node_id/decommission",
            post(edge_decommission),
        )
        .route("/v1/edge/stats", get(edge_stats))
        // Scheduler
        .route("/v1/scheduler/tasks", get(list_tasks).post(submit_task))
        .route("/v1/scheduler/tasks/:task_id", get(get_task))
        .route("/v1/scheduler/tasks/:task_id/cancel", post(cancel_task))
        .route("/v1/scheduler/nodes", post(register_scheduler_node))
        .route("/v1/scheduler/schedule", post(schedule_pending))
        .route("/v1/scheduler/stats", get(scheduler_stats))
        // Metrics
        .route("/v1/metrics", get(metrics))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start the HTTP API server.
///
/// Binds to `config.listen_addr:config.port` and serves the REST API.
pub async fn serve(config: &Config) -> Result<()> {
    let state = Arc::new(AppState {
        config: config.clone(),
        mcp: RwLock::new(McpToolRegistry::new()),
        rag: RwLock::new(RagPipeline::new(RagConfig::default())),
        edge: RwLock::new(EdgeFleetManager::default()),
        scheduler: RwLock::new(TaskScheduler::new()),
        supervisor: RwLock::new(Supervisor::default()),
    });

    let app = router(state);

    let addr = format!("{}:{}", config.listen_addr, config.port);
    info!("listening on {}", addr);

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| DaimonError::ApiError(format!("failed to bind {addr}: {e}")))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| DaimonError::ApiError(format!("server error: {e}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Health response.
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Generic status response for mutation endpoints.
#[derive(Serialize)]
struct StatusResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// RAG ingest request.
#[derive(Deserialize)]
struct RagIngestRequest {
    text: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

/// RAG ingest response.
#[derive(Serialize)]
struct RagIngestResponse {
    chunk_ids: Vec<uuid::Uuid>,
}

/// RAG query request.
#[derive(Deserialize)]
struct RagQueryRequest {
    query: String,
}

/// Edge node registration request.
#[derive(Deserialize)]
struct RegisterEdgeNodeRequest {
    name: String,
    #[serde(default)]
    capabilities: EdgeCapabilities,
    #[serde(default = "default_agent_binary")]
    agent_binary: String,
    #[serde(default = "default_agent_version")]
    agent_version: String,
    #[serde(default)]
    os_version: String,
    #[serde(default)]
    parent_url: String,
}

fn default_agent_binary() -> String {
    "/usr/bin/agent".into()
}
fn default_agent_version() -> String {
    "0.1.0".into()
}

/// Edge node list query.
#[derive(Deserialize, Default)]
struct EdgeNodeQuery {
    status: Option<String>,
}

/// Submit-task request.
#[derive(Deserialize)]
struct SubmitTaskRequest {
    name: String,
    #[serde(default)]
    description: String,
    agent_id: String,
    #[serde(default = "default_priority")]
    priority: u8,
    #[serde(default)]
    resource_requirements: ResourceReq,
}

fn default_priority() -> u8 {
    5
}

/// Register scheduler node request.
#[derive(Deserialize)]
struct RegisterNodeRequest {
    node_id: String,
    total_cpu: f64,
    total_memory_mb: u64,
    #[serde(default = "default_disk")]
    total_disk_mb: u64,
    #[serde(default)]
    gpu_available: bool,
}

fn default_disk() -> u64 {
    102400
}

/// Metrics response.
#[derive(Serialize)]
struct MetricsResponse {
    mcp_tools: usize,
    edge_nodes: u32,
    scheduler_tasks: usize,
    scheduler_running: usize,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /v1/health
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// GET /v1/agents — list supervised agents
async fn list_agents(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sup = state.supervisor.read().await;
    let healths = sup.get_all_health();
    let agents: Vec<serde_json::Value> = healths
        .iter()
        .map(|h| {
            serde_json::json!({
                "agent_id": h.agent_id.to_string(),
                "is_healthy": h.is_healthy,
                "consecutive_failures": h.consecutive_failures,
                "consecutive_successes": h.consecutive_successes,
                "last_response_time_ms": h.last_response_time_ms,
            })
        })
        .collect();
    Json(serde_json::json!({ "agents": agents }))
}

/// GET /v1/agents/:agent_id — get single agent health
async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, DaimonError> {
    let id: agnostik::AgentId = agent_id
        .parse()
        .map_err(|_| DaimonError::InvalidParameter(format!("invalid agent id: {agent_id}")))?;
    let sup = state.supervisor.read().await;
    let health = sup
        .get_health(&id)
        .ok_or_else(|| DaimonError::AgentNotFound(agent_id.clone()))?;
    Ok(Json(serde_json::json!({
        "agent_id": health.agent_id.to_string(),
        "is_healthy": health.is_healthy,
        "consecutive_failures": health.consecutive_failures,
        "consecutive_successes": health.consecutive_successes,
        "last_response_time_ms": health.last_response_time_ms,
    })))
}

/// GET /v1/mcp/tools — list all tools
async fn list_mcp_tools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let reg = state.mcp.read().await;
    Json(reg.manifest())
}

/// POST /v1/mcp/tools — register an external tool
async fn register_mcp_tool(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterMcpToolRequest>,
) -> std::result::Result<(StatusCode, Json<StatusResponse>), DaimonError> {
    state.mcp.write().await.register_external(req)?;
    Ok((
        StatusCode::CREATED,
        Json(StatusResponse {
            ok: true,
            message: None,
        }),
    ))
}

/// DELETE /v1/mcp/tools/:name — deregister an external tool
async fn deregister_mcp_tool(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> std::result::Result<Json<StatusResponse>, DaimonError> {
    state.mcp.write().await.deregister(&name)?;
    Ok(Json(StatusResponse {
        ok: true,
        message: None,
    }))
}

/// POST /v1/mcp/call — call a tool
async fn call_mcp_tool(
    State(state): State<Arc<AppState>>,
    Json(call): Json<McpToolCall>,
) -> std::result::Result<Json<McpToolResult>, DaimonError> {
    let reg = state.mcp.read().await;
    let _tool = reg
        .find_tool(&call.name)
        .ok_or_else(|| DaimonError::AgentNotFound(format!("tool not found: {}", call.name)))?;

    // For external tools, we would forward to the callback URL.
    // For built-in tools, dispatch here. Currently return a placeholder.
    if let Some(url) = reg.external_callback(&call.name) {
        // In production this would POST to the callback URL.
        Ok(Json(McpToolResult::text(format!("dispatched to {url}"))))
    } else {
        Ok(Json(McpToolResult::text(format!(
            "built-in tool '{}' executed",
            call.name
        ))))
    }
}

/// POST /v1/rag/ingest — ingest text
async fn rag_ingest(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RagIngestRequest>,
) -> std::result::Result<(StatusCode, Json<RagIngestResponse>), DaimonError> {
    if req.text.is_empty() {
        return Err(DaimonError::InvalidParameter("text cannot be empty".into()));
    }
    let mut rag = state.rag.write().await;
    let ids = rag.ingest_text(&req.text, req.metadata)?;
    Ok((
        StatusCode::CREATED,
        Json(RagIngestResponse { chunk_ids: ids }),
    ))
}

/// POST /v1/rag/query — query RAG pipeline
async fn rag_query(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RagQueryRequest>,
) -> std::result::Result<Json<crate::rag::RagContext>, DaimonError> {
    if req.query.is_empty() {
        return Err(DaimonError::InvalidParameter(
            "query cannot be empty".into(),
        ));
    }
    let rag = state.rag.read().await;
    let ctx = rag.query_text(&req.query);
    Ok(Json(ctx))
}

/// GET /v1/edge/nodes — list edge nodes
async fn list_edge_nodes(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EdgeNodeQuery>,
) -> Json<serde_json::Value> {
    let mgr = state.edge.read().await;
    let filter = q.status.as_deref().and_then(parse_edge_status);
    let nodes = mgr.list_nodes(filter);
    let list: Vec<serde_json::Value> = nodes
        .iter()
        .filter_map(|n| serde_json::to_value(n).ok())
        .collect();
    Json(serde_json::json!({ "nodes": list }))
}

/// POST /v1/edge/nodes — register a new edge node
async fn register_edge_node(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterEdgeNodeRequest>,
) -> std::result::Result<(StatusCode, Json<serde_json::Value>), DaimonError> {
    let mut mgr = state.edge.write().await;
    let id = mgr.register_node(
        req.name,
        req.capabilities,
        req.agent_binary,
        req.agent_version,
        req.os_version,
        req.parent_url,
    )?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// GET /v1/edge/nodes/:node_id
async fn get_edge_node(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, DaimonError> {
    let mgr = state.edge.read().await;
    let node = mgr
        .get_node(&node_id)
        .ok_or_else(|| DaimonError::AgentNotFound(format!("edge node: {node_id}")))?;
    let val =
        serde_json::to_value(node).map_err(|e| DaimonError::ApiError(format!("serialize: {e}")))?;
    Ok(Json(val))
}

/// POST /v1/edge/nodes/:node_id/heartbeat
async fn edge_heartbeat(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
    Json(hb): Json<HeartbeatData>,
) -> std::result::Result<Json<StatusResponse>, DaimonError> {
    state.edge.write().await.heartbeat(&node_id, hb)?;
    Ok(Json(StatusResponse {
        ok: true,
        message: None,
    }))
}

/// POST /v1/edge/nodes/:node_id/decommission
async fn edge_decommission(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> std::result::Result<Json<StatusResponse>, DaimonError> {
    state.edge.write().await.decommission(&node_id)?;
    Ok(Json(StatusResponse {
        ok: true,
        message: None,
    }))
}

/// GET /v1/edge/stats
async fn edge_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mgr = state.edge.read().await;
    Json(mgr.stats())
}

/// GET /v1/scheduler/tasks — list tasks
async fn list_tasks(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sched = state.scheduler.read().await;
    let stats = sched.stats();
    let pending = sched.pending_tasks();
    let tasks: Vec<serde_json::Value> = pending
        .iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();
    Json(serde_json::json!({
        "stats": stats,
        "pending_tasks": tasks,
    }))
}

/// POST /v1/scheduler/tasks — submit a task
async fn submit_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitTaskRequest>,
) -> std::result::Result<(StatusCode, Json<serde_json::Value>), DaimonError> {
    if req.name.is_empty() {
        return Err(DaimonError::InvalidParameter(
            "task name cannot be empty".into(),
        ));
    }
    let task = ScheduledTask::new(
        req.name,
        req.description,
        req.agent_id,
        req.priority,
        req.resource_requirements,
    );
    let mut sched = state.scheduler.write().await;
    let id = sched.submit_task(task)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "task_id": id })),
    ))
}

/// GET /v1/scheduler/tasks/:task_id
async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, DaimonError> {
    let sched = state.scheduler.read().await;
    let task = sched
        .get_task(&task_id)
        .ok_or_else(|| DaimonError::AgentNotFound(format!("task: {task_id}")))?;
    let val =
        serde_json::to_value(task).map_err(|e| DaimonError::ApiError(format!("serialize: {e}")))?;
    Ok(Json(val))
}

/// POST /v1/scheduler/tasks/:task_id/cancel
async fn cancel_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> std::result::Result<Json<StatusResponse>, DaimonError> {
    state.scheduler.write().await.cancel_task(&task_id)?;
    Ok(Json(StatusResponse {
        ok: true,
        message: None,
    }))
}

/// POST /v1/scheduler/nodes — register a scheduler node
async fn register_scheduler_node(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterNodeRequest>,
) -> std::result::Result<(StatusCode, Json<StatusResponse>), DaimonError> {
    if req.node_id.is_empty() {
        return Err(DaimonError::InvalidParameter(
            "node_id cannot be empty".into(),
        ));
    }
    let node = NodeCapacity::new(
        req.node_id,
        req.total_cpu,
        req.total_memory_mb,
        req.total_disk_mb,
        req.gpu_available,
    );
    state.scheduler.write().await.register_node(node);
    Ok((
        StatusCode::CREATED,
        Json(StatusResponse {
            ok: true,
            message: None,
        }),
    ))
}

/// POST /v1/scheduler/schedule — trigger scheduling of pending tasks
async fn schedule_pending(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut sched = state.scheduler.write().await;
    let decisions = sched.schedule_pending();
    let list: Vec<serde_json::Value> = decisions
        .iter()
        .filter_map(|d| serde_json::to_value(d).ok())
        .collect();
    Json(serde_json::json!({ "decisions": list }))
}

/// GET /v1/scheduler/stats
async fn scheduler_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sched = state.scheduler.read().await;
    Json(sched.stats())
}

/// GET /v1/metrics — aggregate metrics
async fn metrics(State(state): State<Arc<AppState>>) -> Json<MetricsResponse> {
    let mcp_tools = state.mcp.read().await.tool_count();
    let edge_stats = state.edge.read().await.stats();
    let sched_stats = state.scheduler.read().await.stats();
    Json(MetricsResponse {
        mcp_tools,
        edge_nodes: edge_stats.total_nodes,
        scheduler_tasks: sched_stats.total_tasks,
        scheduler_running: sched_stats.running,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_edge_status(s: &str) -> Option<EdgeNodeStatus> {
    match s.to_lowercase().as_str() {
        "online" => Some(EdgeNodeStatus::Online),
        "suspect" => Some(EdgeNodeStatus::Suspect),
        "offline" => Some(EdgeNodeStatus::Offline),
        "updating" => Some(EdgeNodeStatus::Updating),
        "decommissioned" => Some(EdgeNodeStatus::Decommissioned),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            config: Config::default(),
            mcp: RwLock::new(McpToolRegistry::new()),
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

    #[tokio::test]
    async fn health_endpoint() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::get("/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn metrics_endpoint() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::get("/v1/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["mcp_tools"], 0);
    }

    #[tokio::test]
    async fn mcp_tools_empty() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::get("/v1/mcp/tools").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert!(json["tools"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn edge_stats_empty() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::get("/v1/edge/stats").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total_nodes"], 0);
    }

    #[tokio::test]
    async fn scheduler_stats_empty() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::get("/v1/scheduler/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rag_ingest_empty_rejected() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::post("/v1/rag/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"","metadata":null}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rag_query_empty_rejected() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::post("/v1/rag/query")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn submit_task_empty_name_rejected() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::post("/v1/scheduler/tasks")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"","agent_id":"a1","priority":5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
