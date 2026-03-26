//! Agent lifecycle — process spawning, health monitoring, signals, resource tracking.
//!
//! This module provides the runtime representation of an AGNOS agent. The
//! semantic types ([`AgentId`], [`AgentStatus`], [`AgentConfig`], etc.) live in
//! the [`agnostik`] crate; this module adds the stateful process wrapper that
//! manages a running agent's OS process, IPC handle, and (optionally) sandbox.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

use agnostik::{AgentConfig, AgentId, AgentStatus, ResourceUsage, StopReason};
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// AgentHandle — lightweight, cloneable snapshot
// ---------------------------------------------------------------------------

/// Lightweight snapshot of a running agent, safe to clone and send across tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AgentHandle {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Human-readable name.
    pub name: String,
    /// Current lifecycle status.
    pub status: AgentStatus,
    /// Timestamp of agent creation.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp of most recent start (if any).
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Most recent resource usage snapshot.
    pub resource_usage: ResourceUsage,
    /// OS process ID (if spawned).
    pub pid: Option<u32>,
}

// ---------------------------------------------------------------------------
// AgentControl — trait for supervisor interaction
// ---------------------------------------------------------------------------

/// Trait that the supervisor uses to manage agent health and lifecycle.
#[async_trait::async_trait]
pub trait AgentControl: Send + Sync {
    /// Check whether the agent is healthy (process alive, responsive).
    async fn check_health(&self) -> Result<bool>;
    /// Read current resource usage from the OS.
    async fn get_resource_usage(&self) -> Result<ResourceUsage>;
    /// Stop the agent with the given reason.
    async fn stop(&mut self, reason: StopReason) -> Result<()>;
    /// Restart the agent (stop → reset → start).
    async fn restart(&mut self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Agent — the runtime process wrapper
// ---------------------------------------------------------------------------

/// A running (or pending) agent process.
///
/// Wraps a tokio [`Child`] process, tracks lifecycle status via [`RwLock`],
/// and optionally holds a sandbox handle (behind the `sandbox` feature).
pub struct Agent {
    id: AgentId,
    config: AgentConfig,
    status: RwLock<AgentStatus>,
    process: Option<Child>,
    started_at: Option<Instant>,
    #[cfg(feature = "sandbox")]
    _sandbox: Option<kavach::Sandbox>,
}

impl Agent {
    /// Create a new agent from configuration.
    ///
    /// The agent starts in [`AgentStatus::Pending`] and must be explicitly
    /// [`start`](Self::start)ed to spawn the OS process.
    pub fn new(config: AgentConfig) -> Self {
        let id = AgentId::new();

        Self {
            id,
            config,
            status: RwLock::new(AgentStatus::Pending),
            process: None,
            started_at: None,
            #[cfg(feature = "sandbox")]
            _sandbox: None,
        }
    }

    /// Create an agent with a specific ID (useful for restoration / tests).
    #[must_use]
    pub fn with_id(id: AgentId, config: AgentConfig) -> Self {
        Self {
            id,
            config,
            status: RwLock::new(AgentStatus::Pending),
            process: None,
            started_at: None,
            #[cfg(feature = "sandbox")]
            _sandbox: None,
        }
    }

    /// Agent identifier.
    #[must_use]
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Build a cloneable [`AgentHandle`] snapshot of this agent.
    pub async fn handle(&self) -> AgentHandle {
        let pid = self.process.as_ref().and_then(|p| p.id());
        AgentHandle {
            id: self.id,
            name: self.config.name.clone(),
            status: *self.status.read().await,
            created_at: chrono::Utc::now(),
            started_at: self.started_at.map(|_| chrono::Utc::now()),
            resource_usage: self.resource_usage().await,
            pid,
        }
    }

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// Spawn the agent's OS process.
    ///
    /// Transitions from `Pending` or `Stopped` → `Starting` → `Running`.
    /// Returns an error if the agent is in any other state.
    pub async fn start(&mut self) -> Result<()> {
        let mut status = self.status.write().await;

        if *status != AgentStatus::Pending && *status != AgentStatus::Stopped {
            return Err(DaimonError::SupervisorError(format!(
                "agent {} not in a startable state: {:?}",
                self.id, *status
            )));
        }

        *status = AgentStatus::Starting;
        drop(status);

        info!("starting agent {} ({})", self.config.name, self.id);

        let executable = self.find_agent_executable()?;

        let mut cmd = Command::new(&executable);
        cmd.arg("--agent-id")
            .arg(self.id.to_string())
            .arg("--agent-name")
            .arg(&self.config.name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply resource limits via pre_exec (Unix only).
        let max_memory = self.config.resource_limits.max_memory;
        let max_cpu_time = self.config.resource_limits.max_cpu_time;
        if max_memory > 0 || max_cpu_time > 0 {
            #[cfg(unix)]
            unsafe {
                cmd.pre_exec(move || {
                    apply_rlimits(max_memory, max_cpu_time);
                    Ok(())
                });
            }
        }

        let child = cmd.spawn().map_err(|e| {
            DaimonError::SupervisorError(format!(
                "failed to spawn agent process {}: {}",
                executable.display(),
                e
            ))
        })?;

        info!("agent {} started with PID {:?}", self.id, child.id());

        self.process = Some(child);
        self.started_at = Some(Instant::now());
        *self.status.write().await = AgentStatus::Running;

        Ok(())
    }

    /// Stop the agent gracefully (SIGTERM → timeout → SIGKILL).
    pub async fn stop(&mut self, reason: StopReason) -> Result<()> {
        info!("stopping agent {}: {:?}", self.id, reason);

        *self.status.write().await = AgentStatus::Stopping;

        if let Some(ref mut process) = self.process {
            // Send SIGTERM for graceful shutdown.
            #[cfg(unix)]
            {
                if let Some(pid) = process.id() {
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid as i32),
                        nix::sys::signal::Signal::SIGTERM,
                    );
                }
            }

            match tokio::time::timeout(std::time::Duration::from_secs(10), process.wait()).await {
                Ok(Ok(_)) => {
                    info!("agent {} stopped gracefully", self.id);
                }
                Ok(Err(e)) => {
                    warn!("agent {} exit error: {}", self.id, e);
                    process.kill().await.ok();
                }
                Err(_) => {
                    warn!("agent {} shutdown timeout, forcing kill", self.id);
                    process.kill().await.ok();
                }
            }
        }

        *self.status.write().await = AgentStatus::Stopped;
        Ok(())
    }

    /// Pause the agent via SIGSTOP (Unix only).
    pub async fn pause(&mut self) -> Result<()> {
        let mut status = self.status.write().await;
        if *status != AgentStatus::Running {
            return Err(DaimonError::SupervisorError(format!(
                "cannot pause agent {} in state: {:?}",
                self.id, *status
            )));
        }

        #[cfg(unix)]
        if let Some(ref process) = self.process
            && let Some(pid) = process.id()
        {
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGSTOP,
            )
            .map_err(|e| {
                DaimonError::SupervisorError(format!("SIGSTOP failed for agent {}: {}", self.id, e))
            })?;
        }

        *status = AgentStatus::Paused;
        info!("agent {} paused", self.id);
        Ok(())
    }

    /// Resume a paused agent via SIGCONT (Unix only).
    pub async fn resume(&mut self) -> Result<()> {
        let mut status = self.status.write().await;
        if *status != AgentStatus::Paused {
            return Err(DaimonError::SupervisorError(format!(
                "cannot resume agent {} in state: {:?}",
                self.id, *status
            )));
        }

        #[cfg(unix)]
        if let Some(ref process) = self.process
            && let Some(pid) = process.id()
        {
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGCONT,
            )
            .map_err(|e| {
                DaimonError::SupervisorError(format!("SIGCONT failed for agent {}: {}", self.id, e))
            })?;
        }

        *status = AgentStatus::Running;
        info!("agent {} resumed", self.id);
        Ok(())
    }

    /// Check if the agent is currently running.
    pub async fn is_running(&self) -> bool {
        *self.status.read().await == AgentStatus::Running
    }

    // ------------------------------------------------------------------
    // Resource observation (reads /proc/{pid}/)
    // ------------------------------------------------------------------

    /// Read current resource usage from `/proc/{pid}/`.
    ///
    /// Returns zeroed usage if the process has no PID or `/proc` is unavailable.
    pub async fn resource_usage(&self) -> ResourceUsage {
        let pid = match self.process.as_ref().and_then(|p| p.id()) {
            Some(p) => p,
            None => return ResourceUsage::default(),
        };

        ResourceUsage {
            memory_used: read_vm_rss(pid),
            cpu_time_used: read_cpu_time_ms(pid),
            file_descriptors_used: count_fds(pid),
            processes_used: count_threads(pid),
        }
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Locate the executable for this agent type.
    fn find_agent_executable(&self) -> Result<PathBuf> {
        let agent_type = format!("{:?}", self.config.agent_type).to_lowercase();
        let executable_name = format!("agnos-agent-{}-agent", agent_type);

        let search_paths = [
            PathBuf::from("/usr/lib/agnos/agents"),
            PathBuf::from("/opt/agnos/agents"),
            PathBuf::from("./agents"),
        ];

        for path in &search_paths {
            let executable = path.join(&executable_name);
            if executable.exists() {
                return Ok(executable);
            }
        }

        // Default to a generic agent runner.
        Ok(PathBuf::from("/usr/bin/agnos-agent-runner"))
    }
}

// ---------------------------------------------------------------------------
// AgentControl impl
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl AgentControl for Agent {
    async fn check_health(&self) -> Result<bool> {
        if let Some(ref process) = self.process
            && let Some(pid) = process.id()
        {
            #[cfg(unix)]
            {
                let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
                return Ok(alive);
            }
            #[cfg(not(unix))]
            {
                let _ = pid;
                return Ok(true);
            }
        }
        Ok(*self.status.read().await == AgentStatus::Running)
    }

    async fn get_resource_usage(&self) -> Result<ResourceUsage> {
        Ok(self.resource_usage().await)
    }

    async fn stop(&mut self, reason: StopReason) -> Result<()> {
        Agent::stop(self, reason).await
    }

    async fn restart(&mut self) -> Result<()> {
        Agent::stop(self, StopReason::UserRequested).await?;
        *self.status.write().await = AgentStatus::Pending;
        Agent::start(self).await
    }
}

// ---------------------------------------------------------------------------
// /proc helpers (free functions, unit-testable)
// ---------------------------------------------------------------------------

/// Read VmRSS from `/proc/{pid}/status` in bytes.
#[must_use]
fn read_vm_rss(pid: u32) -> u64 {
    let path = format!("/proc/{pid}/status");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| {
            for line in contents.lines() {
                if let Some(val) = line.strip_prefix("VmRSS:") {
                    let kb: u64 = val.split_whitespace().next()?.parse().ok()?;
                    return Some(kb * 1024);
                }
            }
            None
        })
        .unwrap_or(0)
}

/// Read CPU time (utime + stime) from `/proc/{pid}/stat` in milliseconds.
#[must_use]
fn read_cpu_time_ms(pid: u32) -> u64 {
    let path = format!("/proc/{pid}/stat");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| {
            let after_comm = contents.find(')')?.checked_add(2)?;
            let fields: Vec<&str> = contents[after_comm..].split_whitespace().collect();
            let utime: u64 = fields.get(11)?.parse().ok()?;
            let stime: u64 = fields.get(12)?.parse().ok()?;
            let ticks = utime + stime;
            let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
            if ticks_per_sec > 0 {
                Some(ticks * 1000 / ticks_per_sec)
            } else {
                Some(ticks * 10) // fallback: assume 100 Hz
            }
        })
        .unwrap_or(0)
}

/// Count open file descriptors from `/proc/{pid}/fd/`.
#[must_use]
fn count_fds(pid: u32) -> u32 {
    let path = format!("/proc/{pid}/fd");
    std::fs::read_dir(&path)
        .map(|entries| entries.count() as u32)
        .unwrap_or(0)
}

/// Count threads from `/proc/{pid}/task/`.
#[must_use]
fn count_threads(pid: u32) -> u32 {
    let path = format!("/proc/{pid}/task");
    std::fs::read_dir(&path)
        .map(|entries| entries.count() as u32)
        .unwrap_or(1)
}

/// Apply RLIMIT_AS and RLIMIT_CPU to the current process (called from pre_exec).
#[cfg(unix)]
fn apply_rlimits(max_memory: u64, max_cpu_time: u64) {
    use libc::{RLIMIT_AS, RLIMIT_CPU, rlimit, setrlimit};

    if max_memory > 0 {
        let limit = rlimit {
            rlim_cur: max_memory,
            rlim_max: max_memory,
        };
        unsafe {
            setrlimit(RLIMIT_AS, &limit);
        }
    }

    if max_cpu_time > 0 {
        let limit = rlimit {
            rlim_cur: max_cpu_time,
            rlim_max: max_cpu_time,
        };
        unsafe {
            setrlimit(RLIMIT_CPU, &limit);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_handle_serde_roundtrip() {
        let handle = AgentHandle {
            id: AgentId::new(),
            name: "test-agent".into(),
            status: AgentStatus::Running,
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            resource_usage: ResourceUsage::default(),
            pid: Some(1234),
        };

        let json = serde_json::to_string(&handle).unwrap();
        let back: AgentHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, handle.id);
        assert_eq!(back.name, "test-agent");
        assert_eq!(back.status, AgentStatus::Running);
        assert_eq!(back.pid, Some(1234));
    }

    #[test]
    fn agent_handle_default_fields() {
        let handle = AgentHandle {
            id: AgentId::new(),
            name: "pending".into(),
            status: AgentStatus::Pending,
            created_at: chrono::Utc::now(),
            started_at: None,
            resource_usage: ResourceUsage::default(),
            pid: None,
        };

        assert_eq!(handle.status, AgentStatus::Pending);
        assert!(handle.pid.is_none());
        assert!(handle.started_at.is_none());
    }

    #[test]
    fn agent_handle_clone() {
        let handle = AgentHandle {
            id: AgentId::new(),
            name: "clone-test".into(),
            status: AgentStatus::Running,
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            resource_usage: ResourceUsage {
                memory_used: 42,
                cpu_time_used: 100,
                file_descriptors_used: 3,
                processes_used: 1,
            },
            pid: Some(9999),
        };

        let cloned = handle.clone();
        assert_eq!(cloned.id, handle.id);
        assert_eq!(cloned.name, "clone-test");
        assert_eq!(cloned.resource_usage.memory_used, 42);
    }

    #[test]
    fn agent_handle_all_statuses() {
        for status in [
            AgentStatus::Pending,
            AgentStatus::Starting,
            AgentStatus::Running,
            AgentStatus::Paused,
            AgentStatus::Stopping,
            AgentStatus::Stopped,
            AgentStatus::Failed,
        ] {
            let handle = AgentHandle {
                id: AgentId::new(),
                name: format!("agent-{status:?}"),
                status,
                created_at: chrono::Utc::now(),
                started_at: None,
                resource_usage: ResourceUsage::default(),
                pid: None,
            };
            assert_eq!(handle.status, status);
        }
    }

    #[test]
    fn agent_new_is_pending() {
        let agent = Agent::new(AgentConfig::default());
        assert!(agent.process.is_none());
        assert!(agent.started_at.is_none());
    }

    #[test]
    fn agent_with_id() {
        let id = AgentId::new();
        let agent = Agent::with_id(id, AgentConfig::default());
        assert_eq!(agent.id(), id);
    }

    #[test]
    fn agent_ids_unique() {
        let a = Agent::new(AgentConfig::default());
        let b = Agent::new(AgentConfig::default());
        assert_ne!(a.id(), b.id());
    }

    #[tokio::test]
    async fn agent_handle_snapshot() {
        let agent = Agent::new(AgentConfig::default());
        let handle = agent.handle().await;
        assert_eq!(handle.id, agent.id());
        assert_eq!(handle.status, AgentStatus::Pending);
        assert!(handle.pid.is_none());
    }

    #[tokio::test]
    async fn agent_is_running_false_when_pending() {
        let agent = Agent::new(AgentConfig::default());
        assert!(!agent.is_running().await);
    }

    #[tokio::test]
    async fn agent_start_wrong_state() {
        let mut agent = Agent::new(AgentConfig::default());
        // Manually set to Running to trigger the guard.
        *agent.status.write().await = AgentStatus::Running;
        let err = agent.start().await.unwrap_err();
        assert!(err.to_string().contains("not in a startable state"));
    }

    #[tokio::test]
    async fn agent_pause_wrong_state() {
        let agent = Agent::new(AgentConfig::default());
        // Agent is Pending, not Running — pause should fail.
        let mut agent = agent;
        let err = agent.pause().await.unwrap_err();
        assert!(err.to_string().contains("cannot pause"));
    }

    #[tokio::test]
    async fn agent_resume_wrong_state() {
        let mut agent = Agent::new(AgentConfig::default());
        let err = agent.resume().await.unwrap_err();
        assert!(err.to_string().contains("cannot resume"));
    }

    #[tokio::test]
    async fn agent_resource_usage_no_process() {
        let agent = Agent::new(AgentConfig::default());
        let usage = agent.resource_usage().await;
        assert_eq!(usage.memory_used, 0);
        assert_eq!(usage.cpu_time_used, 0);
    }

    #[test]
    fn proc_helpers_nonexistent_pid() {
        let bogus_pid = u32::MAX;
        assert_eq!(read_vm_rss(bogus_pid), 0);
        assert_eq!(read_cpu_time_ms(bogus_pid), 0);
        assert_eq!(count_fds(bogus_pid), 0);
        assert_eq!(count_threads(bogus_pid), 1);
    }

    #[test]
    fn proc_helpers_self() {
        let pid = std::process::id();
        // Our own process should have nonzero RSS and at least 1 thread.
        assert!(read_vm_rss(pid) > 0);
        assert!(count_threads(pid) >= 1);
        assert!(count_fds(pid) > 0);
    }

    #[cfg(unix)]
    #[test]
    fn apply_rlimits_zero_is_noop() {
        // Applying zero limits should not fail.
        apply_rlimits(0, 0);
    }
}
