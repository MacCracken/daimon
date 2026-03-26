//! Error types for the daimon crate.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Errors produced by daimon operations.
///
/// # Examples
///
/// ```
/// # use daimon::DaimonError;
/// let err = DaimonError::AgentNotFound("agent-42".into());
/// assert!(err.to_string().contains("agent-42"));
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DaimonError {
    /// A parameter was invalid.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    /// The requested agent was not found.
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    /// An agent with this ID already exists.
    #[error("agent already exists: {0}")]
    AgentAlreadyExists(String),

    /// Supervisor error during process management.
    #[error("supervisor error: {0}")]
    SupervisorError(String),

    /// IPC communication error.
    #[error("ipc error: {0}")]
    IpcError(String),

    /// Scheduler error.
    #[error("scheduler error: {0}")]
    SchedulerError(String),

    /// Federation error.
    #[error("federation error: {0}")]
    FederationError(String),

    /// HTTP API error.
    #[error("api error: {0}")]
    ApiError(String),

    /// Storage error.
    #[error("storage error: {0}")]
    StorageError(String),

    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Convenience result type for daimon operations.
pub type Result<T> = std::result::Result<T, DaimonError>;

impl IntoResponse for DaimonError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            DaimonError::InvalidParameter(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            DaimonError::AgentNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            DaimonError::AgentAlreadyExists(_) => (StatusCode::CONFLICT, self.to_string()),
            DaimonError::SupervisorError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            DaimonError::IpcError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            DaimonError::SchedulerError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            DaimonError::FederationError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            DaimonError::ApiError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            DaimonError::StorageError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            DaimonError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = serde_json::json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let e = DaimonError::AgentNotFound("agent-42".into());
        assert!(e.to_string().contains("agent-42"));
    }

    #[test]
    fn error_invalid_parameter() {
        let e = DaimonError::InvalidParameter("bad port".into());
        assert!(e.to_string().contains("bad port"));
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e = DaimonError::from(io_err);
        assert!(e.to_string().contains("missing"));
    }

    #[test]
    fn error_into_response_not_found() {
        let e = DaimonError::AgentNotFound("test".into());
        let response = e.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn error_into_response_bad_request() {
        let e = DaimonError::InvalidParameter("bad".into());
        let response = e.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn error_into_response_conflict() {
        let e = DaimonError::AgentAlreadyExists("dup".into());
        let response = e.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
