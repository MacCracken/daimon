//! Multi-node federation — cluster management, peer discovery, scoring,
//! election, and federated vector store.
//!
//! Provides the types and logic for running daimon across multiple nodes
//! with Raft-like leader election, agent placement scoring, and distributed
//! vector search.

use std::collections::HashMap;
use std::net::SocketAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// Role of a node in the federation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NodeRole {
    /// Elected leader.
    Coordinator,
    /// Following the coordinator.
    Follower,
    /// Running for election.
    Candidate,
}

/// Health status of a federation node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NodeStatus {
    /// Receiving heartbeats normally.
    Online,
    /// Missed heartbeats (suspect threshold).
    Suspect,
    /// No heartbeat past dead threshold.
    Dead,
}

/// Hardware capabilities of a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeCapabilities {
    /// Number of CPU cores.
    pub cpu_cores: u32,
    /// Total memory in MiB.
    pub memory_mb: u64,
    /// Number of GPUs.
    pub gpu_count: u32,
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self {
            cpu_cores: 4,
            memory_mb: 8192,
            gpu_count: 0,
        }
    }
}

/// A node in the federation cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FederationNode {
    /// Unique node identifier.
    pub node_id: String,
    /// Human-readable name.
    pub name: String,
    /// Network address.
    pub address: SocketAddr,
    /// Current role.
    pub role: NodeRole,
    /// Health status.
    pub status: NodeStatus,
    /// Last heartbeat timestamp.
    pub last_heartbeat: DateTime<Utc>,
    /// Hardware capabilities.
    pub capabilities: NodeCapabilities,
    /// Current Raft term.
    pub current_term: u64,
    /// Node voted for in current term.
    pub voted_for: Option<String>,
}

impl FederationNode {
    /// Create a new follower node.
    pub fn new(name: String, address: SocketAddr, capabilities: NodeCapabilities) -> Self {
        Self {
            node_id: Uuid::new_v4().to_string(),
            name,
            address,
            role: NodeRole::Follower,
            status: NodeStatus::Online,
            last_heartbeat: Utc::now(),
            capabilities,
            current_term: 0,
            voted_for: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Config & stats
// ---------------------------------------------------------------------------

/// Federation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FederationConfig {
    /// Whether federation is enabled.
    pub enabled: bool,
    /// This node's name.
    pub node_name: String,
    /// Address to bind on.
    pub bind_addr: SocketAddr,
    /// Known peers (name → address).
    pub peers: HashMap<String, SocketAddr>,
    /// Scheduling strategy for agent placement.
    pub scheduling_strategy: SchedulingStrategy,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_name: "node-1".into(),
            bind_addr: "127.0.0.1:8091".parse().unwrap(),
            peers: HashMap::new(),
            scheduling_strategy: SchedulingStrategy::Balanced,
        }
    }
}

/// Scheduling strategy for agent placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum SchedulingStrategy {
    /// Spread load evenly across nodes.
    #[default]
    Balanced,
    /// Pack agents onto fewest nodes.
    Packed,
    /// Maximize isolation between agents.
    Spread,
}

/// Aggregate cluster statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FederationStats {
    /// Total registered nodes.
    pub total_nodes: usize,
    /// Nodes with Online status.
    pub live_nodes: usize,
    /// Nodes with Suspect status.
    pub suspect_nodes: usize,
    /// Nodes with Dead status.
    pub dead_nodes: usize,
    /// Current coordinator (if any).
    pub coordinator_id: Option<String>,
    /// Cluster uptime in seconds.
    pub cluster_uptime_secs: u64,
    /// Active scheduling strategy.
    pub scheduling_strategy: SchedulingStrategy,
}

/// Vote response in leader election.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoteResponse {
    /// ID of the voter.
    pub voter_id: String,
    /// Term the vote is for.
    pub term: u64,
    /// Whether the vote was granted.
    pub granted: bool,
}

// ---------------------------------------------------------------------------
// Agent placement
// ---------------------------------------------------------------------------

/// Resource requirements for placing an agent.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct AgentRequirements {
    /// Required CPU cores.
    pub cpu_cores: u32,
    /// Required memory in MiB.
    pub memory_mb: u64,
    /// Whether a GPU is required.
    pub gpu_required: bool,
    /// Preferred node (locality hint).
    pub preferred_node: Option<String>,
    /// Co-location preference.
    pub affinity_nodes: Vec<String>,
}

/// Weighted score for a node's suitability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeScore {
    /// Node identifier.
    pub node_id: String,
    /// Composite score.
    pub total_score: f64,
    /// Score components.
    pub breakdown: ScoreBreakdown,
}

/// Components of a node score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScoreBreakdown {
    /// Resource headroom (40% weight).
    pub resource_headroom: f64,
    /// Locality match (30% weight).
    pub locality: f64,
    /// Load balance (20% weight).
    pub load_balance: f64,
    /// Affinity match (10% weight).
    pub affinity: f64,
}

/// Scores nodes for agent placement.
#[derive(Debug, Clone, Default)]
pub struct NodeScorer {
    node_loads: HashMap<String, u32>,
}

impl NodeScorer {
    /// Create a new scorer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the agent count for a node.
    pub fn set_load(&mut self, node_id: &str, agent_count: u32) {
        self.node_loads.insert(node_id.to_string(), agent_count);
    }

    /// Get the agent count for a node.
    #[must_use]
    pub fn get_load(&self, node_id: &str) -> u32 {
        self.node_loads.get(node_id).copied().unwrap_or(0)
    }

    /// Score a node for the given requirements.
    #[must_use]
    pub fn score_node(&self, node: &FederationNode, requirements: &AgentRequirements) -> NodeScore {
        let caps = &node.capabilities;

        // Resource headroom (40%)
        let cpu_headroom = if requirements.cpu_cores > 0 {
            (caps.cpu_cores as f64 - requirements.cpu_cores as f64).max(0.0)
                / caps.cpu_cores.max(1) as f64
        } else {
            1.0
        };
        let mem_headroom = if requirements.memory_mb > 0 {
            (caps.memory_mb as f64 - requirements.memory_mb as f64).max(0.0)
                / caps.memory_mb.max(1) as f64
        } else {
            1.0
        };
        let resource_headroom = (cpu_headroom + mem_headroom) / 2.0;

        // Locality (30%)
        let locality = if requirements.preferred_node.as_deref() == Some(&node.node_id) {
            1.0
        } else {
            0.0
        };

        // Load balance (20%)
        let load = self.get_load(&node.node_id) as f64;
        let max_load = self.node_loads.values().copied().max().unwrap_or(1).max(1) as f64;
        let load_balance = 1.0 - (load / max_load);

        // Affinity (10%)
        let affinity = if requirements
            .affinity_nodes
            .iter()
            .any(|n| n == &node.node_id)
        {
            1.0
        } else {
            0.0
        };

        let total_score =
            resource_headroom * 0.4 + locality * 0.3 + load_balance * 0.2 + affinity * 0.1;

        NodeScore {
            node_id: node.node_id.clone(),
            total_score,
            breakdown: ScoreBreakdown {
                resource_headroom,
                locality,
                load_balance,
                affinity,
            },
        }
    }
}

/// Places agents on the best eligible node.
pub struct AgentPlacement {
    scorer: NodeScorer,
}

impl AgentPlacement {
    /// Create with a scorer.
    #[must_use]
    pub fn new(scorer: NodeScorer) -> Self {
        Self { scorer }
    }

    /// Select the best node from the cluster for the given requirements.
    pub fn place_agent(
        &self,
        cluster: &FederationCluster,
        requirements: &AgentRequirements,
    ) -> Result<NodeScore> {
        let live = cluster.get_live_nodes();
        if live.is_empty() {
            return Err(DaimonError::FederationError("no live nodes".into()));
        }

        let mut best: Option<NodeScore> = None;
        for node in &live {
            // Filter out nodes that can't meet GPU requirements.
            if requirements.gpu_required && node.capabilities.gpu_count == 0 {
                continue;
            }
            let score = self.scorer.score_node(node, requirements);
            if best
                .as_ref()
                .is_none_or(|b| score.total_score > b.total_score)
            {
                best = Some(score);
            }
        }

        best.ok_or_else(|| DaimonError::FederationError("no eligible node".into()))
    }
}

// ---------------------------------------------------------------------------
// FederationCluster
// ---------------------------------------------------------------------------

/// Node discovery, health tracking, and Raft-like leader election.
pub struct FederationCluster {
    nodes: HashMap<String, FederationNode>,
    local_node_id: String,
    coordinator_id: Option<String>,
    created_at: DateTime<Utc>,
    votes_received: HashMap<String, Vec<String>>,
    scheduling_strategy: SchedulingStrategy,
    suspect_threshold_secs: i64,
    dead_threshold_secs: i64,
}

impl FederationCluster {
    /// Create a new cluster with the local node.
    pub fn new(local_node: FederationNode) -> Self {
        let local_id = local_node.node_id.clone();
        let mut nodes = HashMap::new();
        nodes.insert(local_id.clone(), local_node);

        Self {
            nodes,
            local_node_id: local_id,
            coordinator_id: None,
            created_at: Utc::now(),
            votes_received: HashMap::new(),
            scheduling_strategy: SchedulingStrategy::Balanced,
            suspect_threshold_secs: 15,
            dead_threshold_secs: 30,
        }
    }

    /// Register a peer node.
    pub fn register_node(&mut self, node: FederationNode) -> Result<()> {
        info!(node_id = %node.node_id, name = %node.name, "registered federation node");
        self.nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    /// Remove a node.
    pub fn remove_node(&mut self, node_id: &str) -> Result<()> {
        self.nodes
            .remove(node_id)
            .ok_or_else(|| DaimonError::FederationError(format!("node not found: {node_id}")))?;
        Ok(())
    }

    /// Get a node by ID.
    #[must_use]
    pub fn get_node(&self, node_id: &str) -> Option<&FederationNode> {
        self.nodes.get(node_id)
    }

    /// Local node ID.
    #[must_use]
    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }

    /// Current coordinator ID.
    #[must_use]
    pub fn coordinator_id(&self) -> Option<&str> {
        self.coordinator_id.as_deref()
    }

    /// Number of nodes in the cluster.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// All nodes.
    #[must_use]
    pub fn all_nodes(&self) -> Vec<&FederationNode> {
        self.nodes.values().collect()
    }

    /// Record a heartbeat for a node.
    pub fn record_heartbeat(&mut self, node_id: &str) -> Result<()> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DaimonError::FederationError(format!("node not found: {node_id}")))?;
        node.last_heartbeat = Utc::now();
        node.status = NodeStatus::Online;
        Ok(())
    }

    /// Update health status of all nodes based on heartbeat age.
    pub fn check_health(&mut self) {
        let now = Utc::now();
        for node in self.nodes.values_mut() {
            let age = (now - node.last_heartbeat).num_seconds();
            if age > self.dead_threshold_secs && node.status != NodeStatus::Dead {
                warn!(node_id = %node.node_id, age_secs = age, "node marked dead");
                node.status = NodeStatus::Dead;
            } else if age > self.suspect_threshold_secs && node.status == NodeStatus::Online {
                debug!(node_id = %node.node_id, age_secs = age, "node marked suspect");
                node.status = NodeStatus::Suspect;
            }
        }
    }

    /// Nodes with Online status.
    #[must_use]
    pub fn get_live_nodes(&self) -> Vec<&FederationNode> {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Online)
            .collect()
    }

    /// Start an election. Returns the new term.
    pub fn start_election(&mut self) -> Result<u64> {
        let node = self
            .nodes
            .get_mut(&self.local_node_id)
            .ok_or_else(|| DaimonError::FederationError("local node not found".into()))?;
        node.role = NodeRole::Candidate;
        node.current_term += 1;
        let term = node.current_term;
        node.voted_for = Some(self.local_node_id.clone());

        self.votes_received
            .insert(self.local_node_id.clone(), vec![self.local_node_id.clone()]);

        info!(term, "started election");
        Ok(term)
    }

    /// Process a vote request from a candidate.
    #[must_use]
    pub fn receive_vote_request(
        &mut self,
        candidate_id: &str,
        candidate_term: u64,
    ) -> VoteResponse {
        let local = match self.nodes.get_mut(&self.local_node_id) {
            Some(n) => n,
            None => {
                return VoteResponse {
                    voter_id: self.local_node_id.clone(),
                    term: 0,
                    granted: false,
                };
            }
        };

        if candidate_term <= local.current_term {
            return VoteResponse {
                voter_id: self.local_node_id.clone(),
                term: local.current_term,
                granted: false,
            };
        }

        // Higher term — step down and grant vote.
        local.current_term = candidate_term;
        local.role = NodeRole::Follower;
        local.voted_for = Some(candidate_id.to_string());

        VoteResponse {
            voter_id: self.local_node_id.clone(),
            term: candidate_term,
            granted: true,
        }
    }

    /// Process a received vote. Returns true if majority reached.
    pub fn receive_vote(&mut self, candidate_id: &str, vote: VoteResponse) -> bool {
        if !vote.granted {
            return false;
        }

        let votes = self
            .votes_received
            .entry(candidate_id.to_string())
            .or_default();
        if !votes.contains(&vote.voter_id) {
            votes.push(vote.voter_id);
        }

        let majority = self.nodes.len() / 2 + 1;
        votes.len() >= majority
    }

    /// Promote a node to coordinator.
    pub fn become_coordinator(&mut self, node_id: &str) -> Result<()> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DaimonError::FederationError(format!("node not found: {node_id}")))?;
        node.role = NodeRole::Coordinator;
        self.coordinator_id = Some(node_id.to_string());
        info!(%node_id, "became coordinator");
        Ok(())
    }

    /// Step a node down to follower.
    pub fn step_down(&mut self, node_id: &str, new_term: u64) -> Result<()> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DaimonError::FederationError(format!("node not found: {node_id}")))?;
        node.role = NodeRole::Follower;
        node.current_term = new_term;
        node.voted_for = None;
        if self.coordinator_id.as_deref() == Some(node_id) {
            self.coordinator_id = None;
        }
        Ok(())
    }

    /// Aggregate cluster statistics.
    #[must_use]
    pub fn stats(&self) -> FederationStats {
        let mut live = 0;
        let mut suspect = 0;
        let mut dead = 0;
        for n in self.nodes.values() {
            match n.status {
                NodeStatus::Online => live += 1,
                NodeStatus::Suspect => suspect += 1,
                NodeStatus::Dead => dead += 1,
            }
        }
        FederationStats {
            total_nodes: self.nodes.len(),
            live_nodes: live,
            suspect_nodes: suspect,
            dead_nodes: dead,
            coordinator_id: self.coordinator_id.clone(),
            cluster_uptime_secs: (Utc::now() - self.created_at).num_seconds().max(0) as u64,
            scheduling_strategy: self.scheduling_strategy,
        }
    }
}

// ---------------------------------------------------------------------------
// Federated vector store
// ---------------------------------------------------------------------------

/// Replication strategy for vector collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum VectorReplicationStrategy {
    /// Every node holds a full copy.
    #[default]
    Full,
    /// Each collection lives on `replication_factor` nodes.
    Partial {
        /// Number of replicas.
        replication_factor: u32,
    },
    /// Each node holds a subset of vectors.
    Sharded,
}

/// A vector collection replica on a specific node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CollectionReplica {
    /// Hosting node.
    pub node_id: String,
    /// Vector API endpoint.
    pub address: SocketAddr,
    /// Number of vectors.
    pub vector_count: usize,
    /// Last synchronization time.
    pub last_synced: DateTime<Utc>,
}

/// Wire-format vector entry for sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VectorSyncEntry {
    /// Vector ID.
    pub id: String,
    /// Embedding values.
    pub embedding: Vec<f64>,
    /// Original content.
    pub content: String,
    /// Metadata.
    pub metadata: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// A search result from a remote node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RemoteSearchResult {
    /// Vector ID.
    pub id: String,
    /// Similarity score.
    pub score: f64,
    /// Content text.
    pub content: String,
    /// Metadata.
    pub metadata: serde_json::Value,
    /// Node that produced this result.
    pub source_node: String,
}

/// Statistics for the federated vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FederatedVectorStats {
    /// Number of collections.
    pub collection_count: usize,
    /// Total replicas across all collections.
    pub total_replicas: usize,
    /// Sum of vector counts across all replicas.
    pub total_vectors_across_replicas: usize,
    /// Number of unique nodes hosting vectors.
    pub nodes_with_vectors: usize,
    /// Active replication strategy.
    pub replication_strategy: VectorReplicationStrategy,
}

/// Federated vector collection management.
///
/// Tracks which nodes hold which vector collections and produces sync
/// messages for cross-node replication and distributed search.
#[derive(Debug, Clone)]
pub struct FederatedVectorStore {
    local_node_id: String,
    collection_map: HashMap<String, Vec<CollectionReplica>>,
    replication_strategy: VectorReplicationStrategy,
    max_remote_results: usize,
}

impl FederatedVectorStore {
    /// Create a new federated vector store.
    #[must_use]
    pub fn new(local_node_id: String, strategy: VectorReplicationStrategy) -> Self {
        Self {
            local_node_id,
            collection_map: HashMap::new(),
            replication_strategy: strategy,
            max_remote_results: 100,
        }
    }

    /// Register a replica for a collection.
    pub fn register_replica(
        &mut self,
        collection: &str,
        node_id: &str,
        address: SocketAddr,
        vector_count: usize,
    ) {
        let replicas = self
            .collection_map
            .entry(collection.to_string())
            .or_default();
        replicas.push(CollectionReplica {
            node_id: node_id.to_string(),
            address,
            vector_count,
            last_synced: Utc::now(),
        });
    }

    /// Remove all replicas for a node.
    pub fn remove_node(&mut self, node_id: &str) {
        for replicas in self.collection_map.values_mut() {
            replicas.retain(|r| r.node_id != node_id);
        }
    }

    /// Remote replicas for a collection (excludes local node).
    #[must_use]
    pub fn remote_replicas(&self, collection: &str) -> Vec<&CollectionReplica> {
        self.collection_map
            .get(collection)
            .map(|rs| {
                rs.iter()
                    .filter(|r| r.node_id != self.local_node_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// All replicas for a collection.
    #[must_use]
    pub fn all_replicas(&self, collection: &str) -> Vec<&CollectionReplica> {
        self.collection_map
            .get(collection)
            .map(|rs| rs.iter().collect())
            .unwrap_or_default()
    }

    /// Sorted list of collection names.
    #[must_use]
    pub fn collections(&self) -> Vec<String> {
        let mut names: Vec<String> = self.collection_map.keys().cloned().collect();
        names.sort();
        names
    }

    /// Number of collections.
    #[must_use]
    pub fn collection_count(&self) -> usize {
        self.collection_map.len()
    }

    /// Merge local and remote search results, dedup by ID, re-rank by score.
    #[must_use]
    pub fn merge_results(
        &self,
        local_results: Vec<RemoteSearchResult>,
        remote_results: Vec<Vec<RemoteSearchResult>>,
        top_k: usize,
    ) -> Vec<RemoteSearchResult> {
        let mut all = local_results;
        for batch in remote_results {
            all.extend(batch.into_iter().take(self.max_remote_results));
        }

        // Dedup by id (keep highest score).
        let mut deduped: HashMap<String, RemoteSearchResult> = HashMap::new();
        for r in all {
            let entry = deduped.entry(r.id.clone()).or_insert(r.clone());
            if r.score > entry.score {
                *entry = r;
            }
        }

        let mut results: Vec<RemoteSearchResult> = deduped.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        results
    }

    /// Aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> FederatedVectorStats {
        let mut total_replicas = 0;
        let mut total_vectors = 0;
        let mut node_set = std::collections::HashSet::new();

        for replicas in self.collection_map.values() {
            total_replicas += replicas.len();
            for r in replicas {
                total_vectors += r.vector_count;
                node_set.insert(r.node_id.clone());
            }
        }

        FederatedVectorStats {
            collection_count: self.collection_map.len(),
            total_replicas,
            total_vectors_across_replicas: total_vectors,
            nodes_with_vectors: node_set.len(),
            replication_strategy: self.replication_strategy,
        }
    }

    /// Replication strategy.
    #[must_use]
    pub fn replication_strategy(&self) -> VectorReplicationStrategy {
        self.replication_strategy
    }

    /// Local node ID.
    #[must_use]
    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn test_node(name: &str, port: u16) -> FederationNode {
        FederationNode::new(name.into(), test_addr(port), NodeCapabilities::default())
    }

    // -- Types serde --

    #[test]
    fn node_role_serde_roundtrip() {
        for role in [
            NodeRole::Coordinator,
            NodeRole::Follower,
            NodeRole::Candidate,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: NodeRole = serde_json::from_str(&json).unwrap();
            assert_eq!(back, role);
        }
    }

    #[test]
    fn node_status_serde_roundtrip() {
        for status in [NodeStatus::Online, NodeStatus::Suspect, NodeStatus::Dead] {
            let json = serde_json::to_string(&status).unwrap();
            let back: NodeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn federation_config_serde_roundtrip() {
        let cfg = FederationConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: FederationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.node_name, cfg.node_name);
    }

    #[test]
    fn scheduling_strategy_default() {
        assert_eq!(SchedulingStrategy::default(), SchedulingStrategy::Balanced);
    }

    // -- FederationCluster --

    #[test]
    fn cluster_new() {
        let node = test_node("local", 8091);
        let cluster = FederationCluster::new(node);
        assert_eq!(cluster.node_count(), 1);
        assert!(cluster.coordinator_id().is_none());
    }

    #[test]
    fn cluster_register_and_get() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        let peer = test_node("peer", 8092);
        let peer_id = peer.node_id.clone();
        cluster.register_node(peer).unwrap();

        assert_eq!(cluster.node_count(), 2);
        assert!(cluster.get_node(&peer_id).is_some());
    }

    #[test]
    fn cluster_remove_node() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        let peer = test_node("peer", 8092);
        let peer_id = peer.node_id.clone();
        cluster.register_node(peer).unwrap();
        cluster.remove_node(&peer_id).unwrap();
        assert_eq!(cluster.node_count(), 1);
    }

    #[test]
    fn cluster_remove_nonexistent() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        assert!(cluster.remove_node("nope").is_err());
    }

    #[test]
    fn cluster_heartbeat() {
        let local = test_node("local", 8091);
        let local_id = local.node_id.clone();
        let mut cluster = FederationCluster::new(local);
        cluster.record_heartbeat(&local_id).unwrap();
        assert_eq!(
            cluster.get_node(&local_id).unwrap().status,
            NodeStatus::Online
        );
    }

    #[test]
    fn cluster_live_nodes() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        let mut dead_node = test_node("dead", 8092);
        dead_node.status = NodeStatus::Dead;
        cluster.register_node(dead_node).unwrap();

        assert_eq!(cluster.get_live_nodes().len(), 1);
    }

    #[test]
    fn cluster_stats() {
        let local = test_node("local", 8091);
        let cluster = FederationCluster::new(local);
        let stats = cluster.stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.live_nodes, 1);
    }

    // -- Election --

    #[test]
    fn cluster_election_increments_term() {
        let local = test_node("local", 8091);
        let local_id = local.node_id.clone();
        let mut cluster = FederationCluster::new(local);
        let term = cluster.start_election().unwrap();
        assert_eq!(term, 1);
        assert_eq!(
            cluster.get_node(&local_id).unwrap().role,
            NodeRole::Candidate
        );
    }

    #[test]
    fn cluster_vote_request_grants_higher_term() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        let vote = cluster.receive_vote_request("candidate-1", 5);
        assert!(vote.granted);
        assert_eq!(vote.term, 5);
    }

    #[test]
    fn cluster_vote_request_rejects_lower_term() {
        let local = test_node("local", 8091);
        let mut cluster = FederationCluster::new(local);
        // Start election to raise term to 1.
        cluster.start_election().unwrap();
        let vote = cluster.receive_vote_request("candidate-2", 0);
        assert!(!vote.granted);
    }

    #[test]
    fn cluster_become_coordinator() {
        let local = test_node("local", 8091);
        let local_id = local.node_id.clone();
        let mut cluster = FederationCluster::new(local);
        cluster.become_coordinator(&local_id).unwrap();
        assert_eq!(cluster.coordinator_id(), Some(local_id.as_str()));
    }

    // -- NodeScorer --

    #[test]
    fn scorer_default_score() {
        let scorer = NodeScorer::new();
        let node = test_node("n1", 8091);
        let score = scorer.score_node(&node, &AgentRequirements::default());
        assert!(score.total_score > 0.0);
    }

    #[test]
    fn scorer_preferred_node_boosts_locality() {
        let scorer = NodeScorer::new();
        let node = test_node("n1", 8091);
        let reqs = AgentRequirements {
            preferred_node: Some(node.node_id.clone()),
            ..Default::default()
        };
        let score = scorer.score_node(&node, &reqs);
        assert_eq!(score.breakdown.locality, 1.0);
    }

    #[test]
    fn scorer_load_balance() {
        let mut scorer = NodeScorer::new();
        let node1 = test_node("n1", 8091);
        let node2 = test_node("n2", 8092);
        scorer.set_load(&node1.node_id, 10);
        scorer.set_load(&node2.node_id, 0);

        let s1 = scorer.score_node(&node1, &AgentRequirements::default());
        let s2 = scorer.score_node(&node2, &AgentRequirements::default());
        assert!(s2.breakdown.load_balance > s1.breakdown.load_balance);
    }

    // -- AgentPlacement --

    #[test]
    fn placement_selects_best() {
        let scorer = NodeScorer::new();
        let placement = AgentPlacement::new(scorer);

        let local = test_node("local", 8091);
        let cluster = FederationCluster::new(local);

        let result = placement
            .place_agent(&cluster, &AgentRequirements::default())
            .unwrap();
        assert!(!result.node_id.is_empty());
    }

    #[test]
    fn placement_no_live_nodes() {
        let scorer = NodeScorer::new();
        let placement = AgentPlacement::new(scorer);

        let mut local = test_node("local", 8091);
        local.status = NodeStatus::Dead;
        let mut cluster = FederationCluster::new(local.clone());
        // Manually set dead (cluster creates as Online).
        if let Some(n) = cluster.nodes.get_mut(&local.node_id) {
            n.status = NodeStatus::Dead;
        }

        assert!(
            placement
                .place_agent(&cluster, &AgentRequirements::default())
                .is_err()
        );
    }

    // -- FederatedVectorStore --

    #[test]
    fn vector_store_register_replica() {
        let mut store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        store.register_replica("docs", "node-1", test_addr(9000), 100);
        assert_eq!(store.all_replicas("docs").len(), 1);
        assert_eq!(store.collection_count(), 1);
    }

    #[test]
    fn vector_store_remote_replicas_exclude_local() {
        let mut store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        store.register_replica("docs", "local", test_addr(9000), 50);
        store.register_replica("docs", "remote", test_addr(9001), 50);
        assert_eq!(store.remote_replicas("docs").len(), 1);
    }

    #[test]
    fn vector_store_remove_node() {
        let mut store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        store.register_replica("docs", "node-1", test_addr(9000), 100);
        store.remove_node("node-1");
        assert!(store.all_replicas("docs").is_empty());
    }

    #[test]
    fn vector_store_merge_results() {
        let store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        let local = vec![RemoteSearchResult {
            id: "a".into(),
            score: 0.9,
            content: "doc a".into(),
            metadata: json!({}),
            source_node: "local".into(),
        }];
        let remote = vec![vec![RemoteSearchResult {
            id: "b".into(),
            score: 0.95,
            content: "doc b".into(),
            metadata: json!({}),
            source_node: "remote".into(),
        }]];

        let merged = store.merge_results(local, remote, 2);
        assert_eq!(merged.len(), 2);
        // Highest score first.
        assert_eq!(merged[0].id, "b");
        assert_eq!(merged[1].id, "a");
    }

    #[test]
    fn vector_store_merge_dedup() {
        let store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        let local = vec![RemoteSearchResult {
            id: "dup".into(),
            score: 0.8,
            content: "doc".into(),
            metadata: json!({}),
            source_node: "local".into(),
        }];
        let remote = vec![vec![RemoteSearchResult {
            id: "dup".into(),
            score: 0.9,
            content: "doc".into(),
            metadata: json!({}),
            source_node: "remote".into(),
        }]];

        let merged = store.merge_results(local, remote, 10);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].score, 0.9); // kept higher score
    }

    #[test]
    fn vector_store_stats() {
        let mut store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        store.register_replica("c1", "n1", test_addr(9000), 100);
        store.register_replica("c1", "n2", test_addr(9001), 100);
        store.register_replica("c2", "n1", test_addr(9000), 50);

        let stats = store.stats();
        assert_eq!(stats.collection_count, 2);
        assert_eq!(stats.total_replicas, 3);
        assert_eq!(stats.total_vectors_across_replicas, 250);
        assert_eq!(stats.nodes_with_vectors, 2);
    }

    #[test]
    fn vector_store_collections_sorted() {
        let mut store = FederatedVectorStore::new("local".into(), VectorReplicationStrategy::Full);
        store.register_replica("zebra", "n1", test_addr(9000), 10);
        store.register_replica("alpha", "n1", test_addr(9000), 10);
        assert_eq!(store.collections(), vec!["alpha", "zebra"]);
    }
}
