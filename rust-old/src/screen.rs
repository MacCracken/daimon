//! Screen capture and recording — per-agent permissions, rate limiting,
//! capture targets, and recording management.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// What to capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CaptureTarget {
    /// Entire screen.
    FullScreen,
    /// A specific window.
    Window {
        /// Window / surface identifier.
        surface_id: String,
    },
    /// A rectangular region.
    Region {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}

/// A screen capture request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScreenCaptureRequest {
    /// What to capture.
    pub target: CaptureTarget,
    /// Output format (e.g. "png", "bmp").
    pub format: String,
    /// Agent requesting the capture.
    pub agent_id: Option<String>,
}

/// A screen capture result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScreenCaptureResponse {
    /// Capture identifier.
    pub id: String,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Image format.
    pub format: String,
    /// Size of encoded data in bytes.
    pub data_size: usize,
    /// RFC 3339 timestamp.
    pub captured_at: String,
    /// Which agent requested this.
    pub requesting_agent: Option<String>,
    /// Base64-encoded image data.
    pub data_base64: String,
}

/// Permission grant for screen capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CapturePermission {
    /// Agent allowed to capture.
    pub agent_id: String,
    /// Allowed target types (e.g. "FullScreen", "Window").
    pub allowed_targets: Vec<String>,
    /// When this permission expires.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Maximum captures per minute.
    pub max_captures_per_minute: u32,
}

/// Request to start a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StartRecordingRequest {
    /// What to record.
    pub target: CaptureTarget,
    /// Frame format.
    pub format: String,
    /// Agent requesting the recording.
    pub agent_id: Option<String>,
    /// Milliseconds between frames.
    pub frame_interval_ms: Option<u32>,
    /// Maximum number of frames.
    pub max_frames: Option<u32>,
    /// Maximum recording duration in seconds.
    pub max_duration_secs: Option<u64>,
}

/// Status of a recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RecordingStatus {
    /// Actively capturing frames.
    Active,
    /// Temporarily paused.
    Paused,
    /// Stopped and finalized.
    Stopped,
}

/// A recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RecordingSession {
    /// Session identifier.
    pub id: String,
    /// Current status.
    pub status: RecordingStatus,
    /// Number of frames captured.
    pub frame_count: u32,
    /// RFC 3339 start timestamp.
    pub started_at: String,
    /// Agent that started the recording.
    pub agent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// CapturePermissionManager
// ---------------------------------------------------------------------------

/// Rate limiter entry.
struct RateEntry {
    timestamps: Vec<Instant>,
}

/// Manages per-agent capture permissions and rate limiting.
pub struct CapturePermissionManager {
    permissions: HashMap<String, CapturePermission>,
    rates: HashMap<String, RateEntry>,
}

impl CapturePermissionManager {
    /// Create a new permission manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
            rates: HashMap::new(),
        }
    }

    /// Grant capture permission to an agent.
    pub fn grant(&mut self, permission: CapturePermission) {
        info!(agent_id = %permission.agent_id, "granted capture permission");
        self.permissions
            .insert(permission.agent_id.clone(), permission);
    }

    /// Revoke capture permission for an agent.
    pub fn revoke(&mut self, agent_id: &str) {
        self.permissions.remove(agent_id);
        self.rates.remove(agent_id);
        info!(agent_id = %agent_id, "revoked capture permission");
    }

    /// Check if an agent has permission to capture. Also enforces rate limiting.
    pub fn check_permission(&mut self, agent_id: &str) -> Result<()> {
        let perm = self.permissions.get(agent_id).ok_or_else(|| {
            DaimonError::InvalidParameter(format!("no capture permission for {agent_id}"))
        })?;

        // Check expiry.
        if let Some(expires_at) = perm.expires_at
            && chrono::Utc::now() > expires_at
        {
            return Err(DaimonError::InvalidParameter(format!(
                "capture permission expired for {agent_id}"
            )));
        }

        // Check rate limit.
        let rate = self.rates.entry(agent_id.to_string()).or_insert(RateEntry {
            timestamps: Vec::new(),
        });

        let window = Duration::from_secs(60);
        let now = Instant::now();
        rate.timestamps.retain(|t| now.duration_since(*t) < window);

        if rate.timestamps.len() as u32 >= perm.max_captures_per_minute {
            warn!(
                agent_id = %agent_id,
                limit = perm.max_captures_per_minute,
                "capture rate limit exceeded"
            );
            return Err(DaimonError::InvalidParameter(format!(
                "rate limit exceeded for {agent_id}: max {} per minute",
                perm.max_captures_per_minute
            )));
        }

        rate.timestamps.push(now);
        Ok(())
    }

    /// List all active permissions.
    #[must_use]
    pub fn list_permissions(&self) -> Vec<&CapturePermission> {
        self.permissions.values().collect()
    }

    /// Get permission for a specific agent.
    #[must_use]
    pub fn get_permission(&self, agent_id: &str) -> Option<&CapturePermission> {
        self.permissions.get(agent_id)
    }
}

impl Default for CapturePermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RecordingManager
// ---------------------------------------------------------------------------

/// Manages active recording sessions.
pub struct RecordingManager {
    sessions: HashMap<String, RecordingSession>,
}

impl RecordingManager {
    /// Create a new recording manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Start a new recording session. Returns the session ID.
    pub fn start(&mut self, agent_id: Option<String>) -> String {
        let id = Uuid::new_v4().to_string();
        let session = RecordingSession {
            id: id.clone(),
            status: RecordingStatus::Active,
            frame_count: 0,
            started_at: chrono::Utc::now().to_rfc3339(),
            agent_id,
        };
        info!(session_id = %id, "recording started");
        self.sessions.insert(id.clone(), session);
        id
    }

    /// Add a frame to a recording.
    pub fn add_frame(&mut self, session_id: &str) -> Result<u32> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            DaimonError::InvalidParameter(format!("recording not found: {session_id}"))
        })?;
        if session.status != RecordingStatus::Active {
            return Err(DaimonError::InvalidParameter(format!(
                "recording {session_id} is not active"
            )));
        }
        session.frame_count += 1;
        Ok(session.frame_count)
    }

    /// Pause a recording.
    pub fn pause(&mut self, session_id: &str) -> Result<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            DaimonError::InvalidParameter(format!("recording not found: {session_id}"))
        })?;
        if session.status != RecordingStatus::Active {
            return Err(DaimonError::InvalidParameter("recording not active".into()));
        }
        session.status = RecordingStatus::Paused;
        debug!(session_id = %session_id, "recording paused");
        Ok(())
    }

    /// Resume a paused recording.
    pub fn resume(&mut self, session_id: &str) -> Result<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            DaimonError::InvalidParameter(format!("recording not found: {session_id}"))
        })?;
        if session.status != RecordingStatus::Paused {
            return Err(DaimonError::InvalidParameter("recording not paused".into()));
        }
        session.status = RecordingStatus::Active;
        debug!(session_id = %session_id, "recording resumed");
        Ok(())
    }

    /// Stop a recording.
    pub fn stop(&mut self, session_id: &str) -> Result<&RecordingSession> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            DaimonError::InvalidParameter(format!("recording not found: {session_id}"))
        })?;
        session.status = RecordingStatus::Stopped;
        info!(session_id = %session_id, frames = session.frame_count, "recording stopped");
        Ok(session)
    }

    /// Get a recording session.
    #[must_use]
    pub fn get(&self, session_id: &str) -> Option<&RecordingSession> {
        self.sessions.get(session_id)
    }

    /// List all sessions.
    #[must_use]
    pub fn list(&self) -> Vec<&RecordingSession> {
        self.sessions.values().collect()
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // -- CaptureTarget serde --

    #[test]
    fn capture_target_serde_roundtrip() {
        let targets = vec![
            CaptureTarget::FullScreen,
            CaptureTarget::Window {
                surface_id: "win-1".into(),
            },
            CaptureTarget::Region {
                x: 10,
                y: 20,
                width: 800,
                height: 600,
            },
        ];
        for target in targets {
            let json = serde_json::to_string(&target).unwrap();
            let back: CaptureTarget = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2);
        }
    }

    // -- CapturePermissionManager --

    #[test]
    fn grant_and_check() {
        let mut mgr = CapturePermissionManager::new();
        mgr.grant(CapturePermission {
            agent_id: "agent-1".into(),
            allowed_targets: vec!["FullScreen".into()],
            expires_at: None,
            max_captures_per_minute: 30,
        });
        assert!(mgr.check_permission("agent-1").is_ok());
    }

    #[test]
    fn check_no_permission() {
        let mut mgr = CapturePermissionManager::new();
        assert!(mgr.check_permission("nobody").is_err());
    }

    #[test]
    fn revoke_permission() {
        let mut mgr = CapturePermissionManager::new();
        mgr.grant(CapturePermission {
            agent_id: "agent-1".into(),
            allowed_targets: vec![],
            expires_at: None,
            max_captures_per_minute: 10,
        });
        mgr.revoke("agent-1");
        assert!(mgr.check_permission("agent-1").is_err());
    }

    #[test]
    fn rate_limiting() {
        let mut mgr = CapturePermissionManager::new();
        mgr.grant(CapturePermission {
            agent_id: "agent-1".into(),
            allowed_targets: vec![],
            expires_at: None,
            max_captures_per_minute: 2,
        });
        mgr.check_permission("agent-1").unwrap();
        mgr.check_permission("agent-1").unwrap();
        assert!(mgr.check_permission("agent-1").is_err());
    }

    #[test]
    fn expired_permission() {
        let mut mgr = CapturePermissionManager::new();
        mgr.grant(CapturePermission {
            agent_id: "agent-1".into(),
            allowed_targets: vec![],
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            max_captures_per_minute: 30,
        });
        let err = mgr.check_permission("agent-1").unwrap_err();
        assert!(err.to_string().contains("expired"));
    }

    #[test]
    fn list_permissions() {
        let mut mgr = CapturePermissionManager::new();
        mgr.grant(CapturePermission {
            agent_id: "a".into(),
            allowed_targets: vec![],
            expires_at: None,
            max_captures_per_minute: 10,
        });
        mgr.grant(CapturePermission {
            agent_id: "b".into(),
            allowed_targets: vec![],
            expires_at: None,
            max_captures_per_minute: 10,
        });
        assert_eq!(mgr.list_permissions().len(), 2);
    }

    // -- RecordingManager --

    #[test]
    fn recording_lifecycle() {
        let mut mgr = RecordingManager::new();
        let id = mgr.start(Some("agent-1".into()));

        assert_eq!(mgr.add_frame(&id).unwrap(), 1);
        assert_eq!(mgr.add_frame(&id).unwrap(), 2);

        mgr.pause(&id).unwrap();
        assert!(mgr.add_frame(&id).is_err()); // paused

        mgr.resume(&id).unwrap();
        assert_eq!(mgr.add_frame(&id).unwrap(), 3);

        let session = mgr.stop(&id).unwrap();
        assert_eq!(session.frame_count, 3);
        assert_eq!(session.status, RecordingStatus::Stopped);
    }

    #[test]
    fn recording_not_found() {
        let mut mgr = RecordingManager::new();
        assert!(mgr.add_frame("nonexistent").is_err());
        assert!(mgr.pause("nonexistent").is_err());
        assert!(mgr.stop("nonexistent").is_err());
    }

    #[test]
    fn recording_list() {
        let mut mgr = RecordingManager::new();
        mgr.start(None);
        mgr.start(None);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn recording_session_serde_roundtrip() {
        let session = RecordingSession {
            id: "sess-1".into(),
            status: RecordingStatus::Active,
            frame_count: 10,
            started_at: "2026-01-01T00:00:00Z".into(),
            agent_id: Some("agent-1".into()),
        };
        let json = serde_json::to_string(&session).unwrap();
        let back: RecordingSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.frame_count, 10);
    }

    #[test]
    fn recording_status_serde_roundtrip() {
        for status in [
            RecordingStatus::Active,
            RecordingStatus::Paused,
            RecordingStatus::Stopped,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: RecordingStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn capture_permission_serde_roundtrip() {
        let perm = CapturePermission {
            agent_id: "agent-1".into(),
            allowed_targets: vec!["FullScreen".into()],
            expires_at: None,
            max_captures_per_minute: 30,
        };
        let json = serde_json::to_string(&perm).unwrap();
        let back: CapturePermission = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "agent-1");
    }
}
