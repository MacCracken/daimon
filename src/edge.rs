//! Edge fleet management — node registration, health tracking, capability
//! routing, OTA updates, and GPU inventory.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Status of an edge node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EdgeNodeStatus {
    /// Active and receiving heartbeats.
    Online,
    /// Missed recent heartbeats.
    Suspect,
    /// Unreachable.
    Offline,
    /// Applying an OTA update.
    Updating,
    /// Permanently retired.
    Decommissioned,
}

/// Hardware capabilities of an edge node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EdgeCapabilities {
    /// CPU architecture (e.g. "aarch64", "x86_64").
    pub arch: String,
    /// Number of CPU cores.
    pub cpu_cores: u32,
    /// Total memory in MiB.
    pub memory_mb: u64,
    /// Total disk in MiB.
    pub disk_mb: u64,
    /// Whether a GPU is available.
    pub has_gpu: bool,
    /// GPU memory in MiB (if present).
    pub gpu_memory_mb: Option<u64>,
    /// GPU compute capability string.
    pub gpu_compute_capability: Option<String>,
    /// Network quality score (0.0–1.0).
    pub network_quality: f64,
    /// Physical location label.
    pub location: Option<String>,
    /// Arbitrary capability tags.
    pub tags: Vec<String>,
}

impl Default for EdgeCapabilities {
    fn default() -> Self {
        Self {
            arch: "x86_64".into(),
            cpu_cores: 4,
            memory_mb: 4096,
            disk_mb: 32768,
            has_gpu: false,
            gpu_memory_mb: None,
            gpu_compute_capability: None,
            network_quality: 1.0,
            location: None,
            tags: Vec::new(),
        }
    }
}

/// An edge node in the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EdgeNode {
    /// Unique node ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Current status.
    pub status: EdgeNodeStatus,
    /// Hardware capabilities.
    pub capabilities: EdgeCapabilities,
    /// Agent binary path.
    pub agent_binary: String,
    /// Agent version string.
    pub agent_version: String,
    /// OS version.
    pub os_version: String,
    /// Parent orchestrator URL.
    pub parent_url: String,
    /// Last heartbeat timestamp.
    pub last_heartbeat: DateTime<Utc>,
    /// Registration timestamp.
    pub registered_at: DateTime<Utc>,
    /// Number of currently running tasks.
    pub active_tasks: u32,
    /// Lifetime completed tasks.
    pub tasks_completed: u64,
    /// Whether TPM attestation has been verified.
    pub tpm_attested: bool,
    /// GPU utilization percentage (if GPU present).
    pub gpu_utilization_pct: Option<f32>,
    /// GPU memory used in MiB.
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU temperature in °C.
    pub gpu_temperature_c: Option<f32>,
    /// Currently loaded model names.
    pub loaded_models: Vec<String>,
}

/// Fleet configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EdgeFleetConfig {
    /// Seconds before a node is marked suspect.
    pub suspect_threshold_secs: u64,
    /// Seconds before a node is marked offline.
    pub offline_threshold_secs: u64,
    /// Maximum nodes in the fleet.
    pub max_nodes: usize,
    /// Whether TPM attestation is required.
    pub require_tpm: bool,
}

impl Default for EdgeFleetConfig {
    fn default() -> Self {
        Self {
            suspect_threshold_secs: 30,
            offline_threshold_secs: 90,
            max_nodes: 1000,
            require_tpm: false,
        }
    }
}

/// Aggregate fleet statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EdgeFleetStats {
    /// Total registered nodes.
    pub total_nodes: u32,
    /// Nodes with Online status.
    pub online: u32,
    /// Nodes with Suspect status.
    pub suspect: u32,
    /// Nodes with Offline status.
    pub offline: u32,
    /// Nodes currently updating.
    pub updating: u32,
    /// Decommissioned nodes.
    pub decommissioned: u32,
    /// Total active tasks across all nodes.
    pub active_tasks: u32,
    /// Lifetime completed tasks.
    pub tasks_completed: u64,
}

/// Heartbeat data from an edge node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HeartbeatData {
    /// Currently running tasks.
    pub active_tasks: u32,
    /// Lifetime completed tasks.
    pub tasks_completed: u64,
    /// GPU utilization percentage.
    pub gpu_utilization_pct: Option<f32>,
    /// GPU memory used in MiB.
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU temperature in °C.
    pub gpu_temperature_c: Option<f32>,
    /// Currently loaded models.
    pub loaded_models: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// EdgeFleetManager
// ---------------------------------------------------------------------------

/// Manages the edge node fleet — registration, heartbeats, health, and stats.
pub struct EdgeFleetManager {
    config: EdgeFleetConfig,
    nodes: HashMap<String, EdgeNode>,
}

impl EdgeFleetManager {
    /// Create a new fleet manager.
    #[must_use]
    pub fn new(config: EdgeFleetConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
        }
    }

    /// Register a new edge node. Returns the assigned node ID.
    pub fn register_node(
        &mut self,
        name: String,
        capabilities: EdgeCapabilities,
        agent_binary: String,
        agent_version: String,
        os_version: String,
        parent_url: String,
    ) -> Result<String> {
        if self.nodes.len() >= self.config.max_nodes {
            return Err(DaimonError::InvalidParameter(format!(
                "fleet full (max {})",
                self.config.max_nodes
            )));
        }
        if name.is_empty() {
            return Err(DaimonError::InvalidParameter(
                "node name cannot be empty".into(),
            ));
        }
        if self.nodes.values().any(|n| n.name == name) {
            return Err(DaimonError::InvalidParameter(format!(
                "duplicate node name: {name}"
            )));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let node = EdgeNode {
            id: id.clone(),
            name: name.clone(),
            status: EdgeNodeStatus::Online,
            capabilities,
            agent_binary,
            agent_version,
            os_version,
            parent_url,
            last_heartbeat: now,
            registered_at: now,
            active_tasks: 0,
            tasks_completed: 0,
            tpm_attested: false,
            gpu_utilization_pct: None,
            gpu_memory_used_mb: None,
            gpu_temperature_c: None,
            loaded_models: Vec::new(),
        };

        info!(node_id = %id, name = %name, "edge node registered");
        self.nodes.insert(id.clone(), node);
        Ok(id)
    }

    /// Process a heartbeat from a node.
    pub fn heartbeat(&mut self, node_id: &str, hb: HeartbeatData) -> Result<()> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DaimonError::AgentNotFound(format!("edge node: {node_id}")))?;

        if node.status == EdgeNodeStatus::Decommissioned {
            return Err(DaimonError::InvalidParameter(format!(
                "node {node_id} is decommissioned"
            )));
        }

        node.last_heartbeat = Utc::now();
        node.status = EdgeNodeStatus::Online;
        node.active_tasks = hb.active_tasks;
        node.tasks_completed = hb.tasks_completed;
        node.gpu_utilization_pct = hb.gpu_utilization_pct;
        node.gpu_memory_used_mb = hb.gpu_memory_used_mb;
        node.gpu_temperature_c = hb.gpu_temperature_c;
        if let Some(models) = hb.loaded_models {
            node.loaded_models = models;
        }

        Ok(())
    }

    /// Update health status based on heartbeat age.
    pub fn check_health(&mut self) {
        let now = Utc::now();
        for node in self.nodes.values_mut() {
            if node.status == EdgeNodeStatus::Decommissioned
                || node.status == EdgeNodeStatus::Updating
            {
                continue;
            }
            let age = (now - node.last_heartbeat).num_seconds().max(0) as u64;
            if age > self.config.offline_threshold_secs && node.status != EdgeNodeStatus::Offline {
                warn!(node_id = %node.id, age_secs = age, "edge node offline");
                node.status = EdgeNodeStatus::Offline;
            } else if age > self.config.suspect_threshold_secs
                && node.status == EdgeNodeStatus::Online
            {
                debug!(node_id = %node.id, age_secs = age, "edge node suspect");
                node.status = EdgeNodeStatus::Suspect;
            }
        }
    }

    /// Decommission a node. Returns the removed node.
    pub fn decommission(&mut self, node_id: &str) -> Result<EdgeNode> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DaimonError::AgentNotFound(format!("edge node: {node_id}")))?;
        node.status = EdgeNodeStatus::Decommissioned;
        info!(node_id = %node_id, "edge node decommissioned");
        Ok(node.clone())
    }

    /// Get a node by ID.
    #[must_use]
    pub fn get_node(&self, node_id: &str) -> Option<&EdgeNode> {
        self.nodes.get(node_id)
    }

    /// List nodes, optionally filtered by status.
    #[must_use]
    pub fn list_nodes(&self, status_filter: Option<EdgeNodeStatus>) -> Vec<&EdgeNode> {
        self.nodes
            .values()
            .filter(|n| status_filter.is_none_or(|s| n.status == s))
            .collect()
    }

    /// Aggregate fleet statistics.
    #[must_use]
    pub fn stats(&self) -> EdgeFleetStats {
        let mut s = EdgeFleetStats {
            total_nodes: self.nodes.len() as u32,
            ..Default::default()
        };
        for n in self.nodes.values() {
            match n.status {
                EdgeNodeStatus::Online => s.online += 1,
                EdgeNodeStatus::Suspect => s.suspect += 1,
                EdgeNodeStatus::Offline => s.offline += 1,
                EdgeNodeStatus::Updating => s.updating += 1,
                EdgeNodeStatus::Decommissioned => s.decommissioned += 1,
            }
            s.active_tasks += n.active_tasks;
            s.tasks_completed += n.tasks_completed;
        }
        s
    }
}

impl Default for EdgeFleetManager {
    fn default() -> Self {
        Self::new(EdgeFleetConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_caps() -> EdgeCapabilities {
        EdgeCapabilities::default()
    }

    fn register_test_node(mgr: &mut EdgeFleetManager, name: &str) -> String {
        mgr.register_node(
            name.into(),
            default_caps(),
            "/usr/bin/agent".into(),
            "0.1.0".into(),
            "Linux 6.x".into(),
            "http://localhost:8090".into(),
        )
        .unwrap()
    }

    #[test]
    fn register_and_get() {
        let mut mgr = EdgeFleetManager::default();
        let id = register_test_node(&mut mgr, "node-1");
        let node = mgr.get_node(&id).unwrap();
        assert_eq!(node.name, "node-1");
        assert_eq!(node.status, EdgeNodeStatus::Online);
    }

    #[test]
    fn register_duplicate_name_rejected() {
        let mut mgr = EdgeFleetManager::default();
        register_test_node(&mut mgr, "dup");
        assert!(
            mgr.register_node(
                "dup".into(),
                default_caps(),
                "bin".into(),
                "0.1".into(),
                "os".into(),
                "url".into()
            )
            .is_err()
        );
    }

    #[test]
    fn register_empty_name_rejected() {
        let mut mgr = EdgeFleetManager::default();
        assert!(
            mgr.register_node(
                "".into(),
                default_caps(),
                "bin".into(),
                "0.1".into(),
                "os".into(),
                "url".into()
            )
            .is_err()
        );
    }

    #[test]
    fn fleet_full() {
        let mut mgr = EdgeFleetManager::new(EdgeFleetConfig {
            max_nodes: 1,
            ..Default::default()
        });
        register_test_node(&mut mgr, "n1");
        assert!(
            mgr.register_node(
                "n2".into(),
                default_caps(),
                "b".into(),
                "v".into(),
                "o".into(),
                "u".into()
            )
            .is_err()
        );
    }

    #[test]
    fn heartbeat_updates_status() {
        let mut mgr = EdgeFleetManager::default();
        let id = register_test_node(&mut mgr, "node-1");
        mgr.heartbeat(
            &id,
            HeartbeatData {
                active_tasks: 5,
                tasks_completed: 100,
                gpu_utilization_pct: Some(80.0),
                gpu_memory_used_mb: Some(2048),
                gpu_temperature_c: Some(65.0),
                loaded_models: Some(vec!["llama".into()]),
            },
        )
        .unwrap();
        let node = mgr.get_node(&id).unwrap();
        assert_eq!(node.active_tasks, 5);
        assert_eq!(node.tasks_completed, 100);
        assert_eq!(node.gpu_utilization_pct, Some(80.0));
        assert_eq!(node.loaded_models, vec!["llama"]);
    }

    #[test]
    fn heartbeat_decommissioned_rejected() {
        let mut mgr = EdgeFleetManager::default();
        let id = register_test_node(&mut mgr, "node-1");
        mgr.decommission(&id).unwrap();
        assert!(mgr.heartbeat(&id, HeartbeatData::default()).is_err());
    }

    #[test]
    fn decommission_node() {
        let mut mgr = EdgeFleetManager::default();
        let id = register_test_node(&mut mgr, "node-1");
        let node = mgr.decommission(&id).unwrap();
        assert_eq!(node.status, EdgeNodeStatus::Decommissioned);
    }

    #[test]
    fn list_nodes_filtered() {
        let mut mgr = EdgeFleetManager::default();
        let id1 = register_test_node(&mut mgr, "online-1");
        let _id2 = register_test_node(&mut mgr, "online-2");
        mgr.decommission(&id1).unwrap();

        assert_eq!(mgr.list_nodes(Some(EdgeNodeStatus::Online)).len(), 1);
        assert_eq!(
            mgr.list_nodes(Some(EdgeNodeStatus::Decommissioned)).len(),
            1
        );
        assert_eq!(mgr.list_nodes(None).len(), 2);
    }

    #[test]
    fn stats_counts() {
        let mut mgr = EdgeFleetManager::default();
        register_test_node(&mut mgr, "n1");
        register_test_node(&mut mgr, "n2");
        let s = mgr.stats();
        assert_eq!(s.total_nodes, 2);
        assert_eq!(s.online, 2);
    }

    #[test]
    fn edge_node_serde_roundtrip() {
        let mut mgr = EdgeFleetManager::default();
        let id = register_test_node(&mut mgr, "serde-test");
        let node = mgr.get_node(&id).unwrap();
        let json = serde_json::to_string(node).unwrap();
        let back: EdgeNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "serde-test");
    }

    #[test]
    fn edge_fleet_stats_serde_roundtrip() {
        let stats = EdgeFleetStats {
            total_nodes: 5,
            online: 3,
            ..Default::default()
        };
        let json = serde_json::to_string(&stats).unwrap();
        let back: EdgeFleetStats = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total_nodes, 5);
    }

    #[test]
    fn edge_capabilities_default() {
        let caps = EdgeCapabilities::default();
        assert_eq!(caps.arch, "x86_64");
        assert!(!caps.has_gpu);
    }

    #[test]
    fn edge_config_serde_roundtrip() {
        let cfg = EdgeFleetConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: EdgeFleetConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.max_nodes, cfg.max_nodes);
    }
}
