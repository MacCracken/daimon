//! HTTP API server — axum-based REST API on port 8090.
//!
//! Endpoints: /v1/health, /v1/agents, /v1/metrics, /v1/mcp, /v1/rag, /v1/edge, etc.

use crate::config::Config;
use crate::error::Result;

/// Shared state for the API server.
#[derive(Debug, Clone)]
pub struct ApiState {
    /// Service configuration.
    pub config: Config,
}

/// Start the HTTP API server.
///
/// Binds to `config.listen_addr:config.port` and serves the REST API.
pub async fn serve(config: &Config) -> Result<()> {
    let _state = ApiState {
        config: config.clone(),
    };
    // TODO: Build axum router, bind, and serve.
    Ok(())
}
