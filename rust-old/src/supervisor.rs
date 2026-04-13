//! Process supervisor — health monitoring, circuit breaker, resource quotas,
//! output capture, and recovery.
//!
//! The [`Supervisor`] manages a set of agents, periodically checking health
//! and resource usage. It delegates to [`CircuitBreaker`] for failure
//! management and [`OutputCapture`] for stdout/stderr ring buffers.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use agnostik::{AgentId, ResourceUsage};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::agent::AgentControl;
use crate::error::Result;

// ---------------------------------------------------------------------------
// CircuitBreaker
// ---------------------------------------------------------------------------

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CircuitState {
    /// Normal operation — requests allowed.
    Closed,
    /// Failures exceeded threshold — requests blocked.
    Open,
    /// Recovery window — limited requests allowed.
    HalfOpen,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CircuitBreakerConfig {
    /// Failures before opening the circuit.
    pub failure_threshold: u32,
    /// Milliseconds before transitioning Open → HalfOpen.
    pub recovery_timeout_ms: u64,
    /// Successes needed in HalfOpen to close the circuit.
    pub half_open_max_attempts: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_ms: 30_000,
            half_open_max_attempts: 3,
        }
    }
}

/// Circuit breaker for failure management.
///
/// State machine: Closed → Open → HalfOpen → Closed.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max: u32,
    last_failure_time: Option<Instant>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    #[must_use]
    pub fn new(failure_threshold: u32, recovery_timeout: Duration, half_open_max: u32) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            recovery_timeout,
            half_open_max,
            last_failure_time: None,
        }
    }

    /// Create from a config struct.
    #[must_use]
    pub fn from_config(config: &CircuitBreakerConfig) -> Self {
        Self::new(
            config.failure_threshold,
            Duration::from_millis(config.recovery_timeout_ms),
            config.half_open_max_attempts,
        )
    }

    /// Record a successful operation.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.half_open_max {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                    debug!("circuit breaker closed after recovery");
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed operation.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    warn!(failures = self.failure_count, "circuit breaker opened");
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.success_count = 0;
                warn!("circuit breaker re-opened from half-open");
            }
            CircuitState::Open => {}
        }
    }

    /// Check whether an operation should be allowed.
    ///
    /// Transitions Open → HalfOpen if the recovery timeout has elapsed.
    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                if let Some(last) = self.last_failure_time
                    && last.elapsed() >= self.recovery_timeout
                {
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                    debug!("circuit breaker transitioning to half-open");
                    return true;
                }
                false
            }
        }
    }

    /// Reset to closed state.
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.last_failure_time = None;
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Number of consecutive failures.
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// When the last failure occurred.
    #[must_use]
    pub fn last_failure_time(&self) -> Option<Instant> {
        self.last_failure_time
    }
}

// ---------------------------------------------------------------------------
// ResourceQuota
// ---------------------------------------------------------------------------

/// Configurable resource thresholds for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResourceQuota {
    /// Memory usage warning threshold (0.0-1.0).
    pub memory_warn_pct: f64,
    /// Memory usage kill threshold (0.0-1.0).
    pub memory_kill_pct: f64,
    /// CPU rate warning threshold (0.0-1.0).
    pub cpu_throttle_pct: f64,
    /// Memory limit in bytes.
    pub memory_limit: u64,
    /// CPU time limit in milliseconds.
    pub cpu_time_limit: u64,
}

impl ResourceQuota {
    /// Create a quota with the given limits and default thresholds.
    #[must_use]
    pub fn from_limits(memory_limit: u64, cpu_time_limit: u64) -> Self {
        Self {
            memory_warn_pct: 0.80,
            memory_kill_pct: 0.95,
            cpu_throttle_pct: 0.90,
            memory_limit,
            cpu_time_limit,
        }
    }
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self::from_limits(1024 * 1024 * 1024, 3600 * 1000) // 1 GiB, 1 hour
    }
}

// ---------------------------------------------------------------------------
// AgentHealth
// ---------------------------------------------------------------------------

/// Health monitoring snapshot for an agent.
#[derive(Debug, Clone)]
pub struct AgentHealth {
    /// Agent identifier.
    pub agent_id: AgentId,
    /// Whether the agent is currently considered healthy.
    pub is_healthy: bool,
    /// Consecutive failed health checks.
    pub consecutive_failures: u32,
    /// Consecutive successful health checks.
    pub consecutive_successes: u32,
    /// When the last check was performed.
    pub last_check: Instant,
    /// Response time of the last check in milliseconds.
    pub last_response_time_ms: u64,
    /// Most recent resource usage snapshot.
    pub resource_usage: ResourceUsage,
}

impl AgentHealth {
    /// Create a new health record for an agent (initially healthy).
    #[must_use]
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            is_healthy: true,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_check: Instant::now(),
            last_response_time_ms: 0,
            resource_usage: ResourceUsage::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// HealthCheckConfig
// ---------------------------------------------------------------------------

/// Configuration for health check timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HealthCheckConfig {
    /// Interval between health checks.
    #[serde(with = "duration_secs")]
    pub interval: Duration,
    /// Timeout per individual check.
    #[serde(with = "duration_secs")]
    pub timeout: Duration,
    /// Failed checks before marking unhealthy.
    pub unhealthy_threshold: u32,
    /// Successful checks before marking healthy.
    pub healthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            unhealthy_threshold: 3,
            healthy_threshold: 2,
        }
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> std::result::Result<S::Ok, S::Error> {
        d.as_secs().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

// ---------------------------------------------------------------------------
// OutputCapture
// ---------------------------------------------------------------------------

/// Stream type for captured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// A single captured output line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OutputLine {
    /// RFC 3339 timestamp.
    pub timestamp: String,
    /// Which stream produced this line.
    pub stream: OutputStream,
    /// The content of the line.
    pub content: String,
}

/// Ring buffer for agent stdout/stderr capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputCapture {
    lines: Vec<OutputLine>,
    max_lines: usize,
}

impl OutputCapture {
    /// Create a new capture buffer with the given capacity.
    #[must_use]
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }

    /// Push a line into the buffer. Evicts the oldest line if full.
    pub fn push(&mut self, stream: OutputStream, content: String) {
        if self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(OutputLine {
            timestamp: chrono::Utc::now().to_rfc3339(),
            stream,
            content,
        });
    }

    /// Get the last `n` lines.
    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<&OutputLine> {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].iter().collect()
    }

    /// Get all captured lines.
    #[must_use]
    pub fn all(&self) -> Vec<&OutputLine> {
        self.lines.iter().collect()
    }

    /// Filter by stream type.
    #[must_use]
    pub fn filter_stream(&self, stream: OutputStream) -> Vec<&OutputLine> {
        self.lines.iter().filter(|l| l.stream == stream).collect()
    }

    /// Clear all captured output.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Number of captured lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

/// Agent process supervisor.
///
/// Tracks health, resource usage, quotas, and circuit breakers for a set of
/// managed agents. Call [`check_health`](Self::check_health) periodically.
pub struct Supervisor {
    health: HashMap<AgentId, AgentHealth>,
    quotas: HashMap<AgentId, ResourceQuota>,
    breakers: HashMap<AgentId, CircuitBreaker>,
    config: HealthCheckConfig,
}

impl Supervisor {
    /// Create a new supervisor with the given health check config.
    #[must_use]
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            health: HashMap::new(),
            quotas: HashMap::new(),
            breakers: HashMap::new(),
            config,
        }
    }

    /// Register an agent for supervision.
    pub fn register_agent(&mut self, agent_id: AgentId) {
        self.health.insert(agent_id, AgentHealth::new(agent_id));
        self.quotas.insert(agent_id, ResourceQuota::default());
        self.breakers.insert(
            agent_id,
            CircuitBreaker::from_config(&CircuitBreakerConfig::default()),
        );
        info!(%agent_id, "agent registered for supervision");
    }

    /// Unregister an agent.
    pub fn unregister_agent(&mut self, agent_id: &AgentId) {
        self.health.remove(agent_id);
        self.quotas.remove(agent_id);
        self.breakers.remove(agent_id);
        info!(%agent_id, "agent unregistered from supervision");
    }

    /// Set a resource quota for an agent.
    pub fn set_quota(&mut self, agent_id: AgentId, quota: ResourceQuota) {
        self.quotas.insert(agent_id, quota);
    }

    /// Get the current quota for an agent.
    #[must_use]
    pub fn get_quota(&self, agent_id: &AgentId) -> Option<&ResourceQuota> {
        self.quotas.get(agent_id)
    }

    /// Get health status for an agent.
    #[must_use]
    pub fn get_health(&self, agent_id: &AgentId) -> Option<&AgentHealth> {
        self.health.get(agent_id)
    }

    /// Get all agent health statuses.
    #[must_use]
    pub fn get_all_health(&self) -> Vec<&AgentHealth> {
        self.health.values().collect()
    }

    /// Perform a health check on a single agent via its [`AgentControl`] impl.
    pub async fn check_health<A: AgentControl>(
        &mut self,
        agent_id: &AgentId,
        agent: &A,
    ) -> Result<bool> {
        let start = Instant::now();

        let healthy = match tokio::time::timeout(self.config.timeout, agent.check_health()).await {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                debug!(%agent_id, err = %e, "health check failed");
                false
            }
            Err(_) => {
                warn!(%agent_id, "health check timed out");
                false
            }
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if let Some(health) = self.health.get_mut(agent_id) {
            health.last_check = Instant::now();
            health.last_response_time_ms = elapsed_ms;

            if healthy {
                health.consecutive_failures = 0;
                health.consecutive_successes += 1;
                if health.consecutive_successes >= self.config.healthy_threshold {
                    health.is_healthy = true;
                }
                if let Some(cb) = self.breakers.get_mut(agent_id) {
                    cb.record_success();
                }
            } else {
                health.consecutive_successes = 0;
                health.consecutive_failures += 1;
                if health.consecutive_failures >= self.config.unhealthy_threshold {
                    health.is_healthy = false;
                }
                if let Some(cb) = self.breakers.get_mut(agent_id) {
                    cb.record_failure();
                }
            }

            // Update resource usage.
            if let Ok(usage) = agent.get_resource_usage().await {
                health.resource_usage = usage;
            }
        }

        Ok(healthy)
    }

    /// Check whether the circuit breaker allows operations for an agent.
    pub fn can_execute(&mut self, agent_id: &AgentId) -> bool {
        self.breakers
            .get_mut(agent_id)
            .is_some_and(|cb| cb.can_execute())
    }

    /// Get the circuit breaker state for an agent.
    #[must_use]
    pub fn circuit_state(&self, agent_id: &AgentId) -> Option<CircuitState> {
        self.breakers.get(agent_id).map(|cb| cb.state())
    }

    /// Check resource quota for an agent. Returns a warning message if exceeded.
    #[must_use]
    pub fn check_quota(&self, agent_id: &AgentId) -> Option<String> {
        let quota = self.quotas.get(agent_id)?;
        let health = self.health.get(agent_id)?;
        let usage = &health.resource_usage;

        if quota.memory_limit > 0 {
            let pct = usage.memory_used as f64 / quota.memory_limit as f64;
            if pct >= quota.memory_kill_pct {
                return Some(format!(
                    "agent {} memory at {:.0}% — exceeds kill threshold",
                    agent_id,
                    pct * 100.0
                ));
            }
            if pct >= quota.memory_warn_pct {
                return Some(format!(
                    "agent {} memory at {:.0}% — exceeds warning threshold",
                    agent_id,
                    pct * 100.0
                ));
            }
        }

        None
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new(HealthCheckConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_id(n: u8) -> AgentId {
        AgentId(uuid::Uuid::from_bytes([n; 16]))
    }

    // -- CircuitBreaker --

    #[test]
    fn circuit_starts_closed() {
        let cb = CircuitBreaker::from_config(&CircuitBreakerConfig::default());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn circuit_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(10), 2);
        assert!(cb.can_execute());
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());
    }

    #[test]
    fn circuit_transitions_to_half_open() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(0), 1);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        // Recovery timeout is 0ms, so it should immediately go to HalfOpen.
        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn circuit_closes_after_half_open_successes() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(0), 2);
        cb.record_failure();
        cb.can_execute(); // → HalfOpen
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn circuit_reopens_on_half_open_failure() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(0), 3);
        cb.record_failure();
        cb.can_execute(); // → HalfOpen
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn circuit_reset() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(60), 1);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn circuit_success_resets_failure_count() {
        let mut cb = CircuitBreaker::new(5, Duration::from_secs(10), 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
    }

    // -- ResourceQuota --

    #[test]
    fn quota_defaults() {
        let q = ResourceQuota::default();
        assert!(q.memory_warn_pct > 0.0);
        assert!(q.memory_kill_pct > q.memory_warn_pct);
        assert!(q.memory_limit > 0);
    }

    #[test]
    fn quota_from_limits() {
        let q = ResourceQuota::from_limits(1024, 5000);
        assert_eq!(q.memory_limit, 1024);
        assert_eq!(q.cpu_time_limit, 5000);
    }

    #[test]
    fn quota_serde_roundtrip() {
        let q = ResourceQuota::default();
        let json = serde_json::to_string(&q).unwrap();
        let back: ResourceQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(back.memory_limit, q.memory_limit);
    }

    // -- AgentHealth --

    #[test]
    fn health_new_is_healthy() {
        let h = AgentHealth::new(test_id(1));
        assert!(h.is_healthy);
        assert_eq!(h.consecutive_failures, 0);
        assert_eq!(h.consecutive_successes, 0);
    }

    // -- HealthCheckConfig --

    #[test]
    fn health_config_defaults() {
        let c = HealthCheckConfig::default();
        assert_eq!(c.interval, Duration::from_secs(30));
        assert_eq!(c.timeout, Duration::from_secs(5));
        assert_eq!(c.unhealthy_threshold, 3);
        assert_eq!(c.healthy_threshold, 2);
    }

    #[test]
    fn health_config_serde_roundtrip() {
        let c = HealthCheckConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: HealthCheckConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.unhealthy_threshold, c.unhealthy_threshold);
    }

    // -- OutputCapture --

    #[test]
    fn output_capture_push_and_tail() {
        let mut cap = OutputCapture::new(5);
        cap.push(OutputStream::Stdout, "line1".into());
        cap.push(OutputStream::Stderr, "line2".into());
        cap.push(OutputStream::Stdout, "line3".into());

        assert_eq!(cap.len(), 3);
        let tail = cap.tail(2);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].content, "line2");
        assert_eq!(tail[1].content, "line3");
    }

    #[test]
    fn output_capture_ring_buffer() {
        let mut cap = OutputCapture::new(3);
        for i in 0..5 {
            cap.push(OutputStream::Stdout, format!("line{i}"));
        }
        assert_eq!(cap.len(), 3);
        let all = cap.all();
        assert_eq!(all[0].content, "line2");
        assert_eq!(all[2].content, "line4");
    }

    #[test]
    fn output_capture_filter_stream() {
        let mut cap = OutputCapture::new(10);
        cap.push(OutputStream::Stdout, "out".into());
        cap.push(OutputStream::Stderr, "err".into());
        cap.push(OutputStream::Stdout, "out2".into());

        assert_eq!(cap.filter_stream(OutputStream::Stdout).len(), 2);
        assert_eq!(cap.filter_stream(OutputStream::Stderr).len(), 1);
    }

    #[test]
    fn output_capture_clear() {
        let mut cap = OutputCapture::new(10);
        cap.push(OutputStream::Stdout, "data".into());
        cap.clear();
        assert!(cap.is_empty());
    }

    // -- Supervisor --

    #[test]
    fn supervisor_register_unregister() {
        let mut sup = Supervisor::default();
        let id = test_id(1);
        sup.register_agent(id);
        assert!(sup.get_health(&id).is_some());
        assert!(sup.get_quota(&id).is_some());

        sup.unregister_agent(&id);
        assert!(sup.get_health(&id).is_none());
    }

    #[test]
    fn supervisor_set_quota() {
        let mut sup = Supervisor::default();
        let id = test_id(1);
        sup.register_agent(id);
        sup.set_quota(id, ResourceQuota::from_limits(2048, 1000));
        assert_eq!(sup.get_quota(&id).unwrap().memory_limit, 2048);
    }

    #[test]
    fn supervisor_all_health() {
        let mut sup = Supervisor::default();
        sup.register_agent(test_id(1));
        sup.register_agent(test_id(2));
        assert_eq!(sup.get_all_health().len(), 2);
    }

    #[test]
    fn supervisor_circuit_state() {
        let mut sup = Supervisor::default();
        let id = test_id(1);
        sup.register_agent(id);
        assert_eq!(sup.circuit_state(&id), Some(CircuitState::Closed));
        assert!(sup.can_execute(&id));
    }

    #[test]
    fn supervisor_check_quota_under_limit() {
        let mut sup = Supervisor::default();
        let id = test_id(1);
        sup.register_agent(id);
        // Default usage is 0 bytes, so no warning.
        assert!(sup.check_quota(&id).is_none());
    }

    #[test]
    fn supervisor_check_quota_exceeded() {
        let mut sup = Supervisor::default();
        let id = test_id(1);
        sup.register_agent(id);
        sup.set_quota(id, ResourceQuota::from_limits(1000, 5000));

        // Simulate high memory usage.
        if let Some(h) = sup.health.get_mut(&id) {
            h.resource_usage.memory_used = 960; // 96%
        }
        let warning = sup.check_quota(&id);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("kill threshold"));
    }

    // -- CircuitState serde --

    #[test]
    fn circuit_state_serde_roundtrip() {
        for state in [
            CircuitState::Closed,
            CircuitState::Open,
            CircuitState::HalfOpen,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            let back: CircuitState = serde_json::from_str(&json).unwrap();
            assert_eq!(back, state);
        }
    }

    // -- CircuitBreakerConfig serde --

    #[test]
    fn circuit_config_serde_roundtrip() {
        let cfg = CircuitBreakerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: CircuitBreakerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.failure_threshold, cfg.failure_threshold);
    }

    // -- OutputLine serde --

    #[test]
    fn output_line_serde_roundtrip() {
        let line = OutputLine {
            timestamp: "2026-01-01T00:00:00Z".into(),
            stream: OutputStream::Stderr,
            content: "error msg".into(),
        };
        let json = serde_json::to_string(&line).unwrap();
        let back: OutputLine = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stream, OutputStream::Stderr);
        assert_eq!(back.content, "error msg");
    }

    #[test]
    fn output_capture_serde_roundtrip() {
        let mut cap = OutputCapture::new(10);
        cap.push(OutputStream::Stdout, "hello".into());
        cap.push(OutputStream::Stderr, "error".into());
        let json = serde_json::to_string(&cap).unwrap();
        let back: OutputCapture = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
    }
}
