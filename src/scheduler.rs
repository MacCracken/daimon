//! Priority-aware task scheduler with cron triggers.
//!
//! Tasks are submitted, queued by priority, and assigned to nodes based on
//! resource availability. Supports preemption, cancellation, deadline-aware
//! scheduling, and cron-like recurring task triggers.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// AcceleratorRequirement (local, mirrors ai-hwaccel)
// ---------------------------------------------------------------------------

/// Hardware accelerator requirement for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum AcceleratorRequirement {
    /// No accelerator needed.
    #[default]
    None,
    /// Requires a GPU.
    Gpu,
    /// Requires a TPU with at least `min_chips` chips.
    Tpu {
        /// Minimum number of TPU chips.
        min_chips: u32,
    },
    /// Either GPU or TPU.
    GpuOrTpu,
    /// Any accelerator (not CPU-only).
    AnyAccelerator,
}

// ---------------------------------------------------------------------------
// ResourceReq
// ---------------------------------------------------------------------------

/// Resource requirements for a scheduled task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResourceReq {
    /// Required CPU cores (fractional allowed).
    pub cpu_cores: f64,
    /// Required memory in megabytes.
    pub memory_mb: u64,
    /// Hardware accelerator requirement.
    pub accelerator: AcceleratorRequirement,
    /// Whether network access is required.
    pub network: bool,
    /// Required disk space in megabytes.
    pub disk_mb: u64,
}

impl Default for ResourceReq {
    fn default() -> Self {
        Self {
            cpu_cores: 1.0,
            memory_mb: 256,
            accelerator: AcceleratorRequirement::None,
            network: false,
            disk_mb: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskStatus
// ---------------------------------------------------------------------------

/// Status of a scheduled task (state machine).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskStatus {
    /// Waiting in the queue.
    Queued,
    /// Assigned to a node but not yet started.
    Scheduled,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed(String),
    /// Cancelled by user or system.
    Cancelled,
    /// Evicted by a higher-priority task.
    Preempted,
}

impl TaskStatus {
    /// Whether transitioning from `self` to `to` is valid.
    #[must_use]
    pub fn valid_transition(&self, to: &TaskStatus) -> bool {
        use TaskStatus::*;
        matches!(
            (self, to),
            (Queued, Scheduled)
                | (Queued, Cancelled)
                | (Scheduled, Running)
                | (Scheduled, Cancelled)
                | (Scheduled, Queued)
                | (Running, Completed)
                | (Running, Failed(_))
                | (Running, Cancelled)
                | (Running, Preempted)
                | (Preempted, Queued)
        )
    }
}

// ---------------------------------------------------------------------------
// TaskPriority
// ---------------------------------------------------------------------------

/// Priority classification for tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TaskPriority {
    /// Priority 1-3.
    Normal,
    /// Priority 4-6.
    High,
    /// Priority 7-9.
    Critical,
    /// Priority 10.
    Emergency,
}

impl TaskPriority {
    /// Derive from a numeric priority (1-10).
    #[must_use]
    pub fn from_numeric(p: u8) -> Self {
        match p {
            1..=3 => Self::Normal,
            4..=6 => Self::High,
            7..=9 => Self::Critical,
            10 => Self::Emergency,
            _ => Self::Normal,
        }
    }

    /// Whether `self` can preempt `other` (strictly higher).
    #[must_use]
    pub fn can_preempt(&self, other: &TaskPriority) -> bool {
        *self > *other
    }
}

// ---------------------------------------------------------------------------
// ScheduledTask
// ---------------------------------------------------------------------------

/// A task managed by the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScheduledTask {
    /// Unique task identifier.
    pub task_id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the task.
    pub description: String,
    /// Agent that submitted the task.
    pub agent_id: String,
    /// Numeric priority 1-10 (10 = highest).
    pub priority: u8,
    /// Resource requirements.
    pub resource_requirements: ResourceReq,
    /// Current status.
    pub status: TaskStatus,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task was assigned to a node.
    pub scheduled_at: Option<DateTime<Utc>>,
    /// When execution began.
    pub started_at: Option<DateTime<Utc>>,
    /// When execution completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Deadline for completion.
    pub deadline: Option<DateTime<Utc>>,
    /// Preferred node for execution.
    pub node_preference: Option<String>,
}

impl ScheduledTask {
    /// Create a new task in `Queued` status. Priority is clamped to 1-10.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        agent_id: impl Into<String>,
        priority: u8,
        resource_requirements: ResourceReq,
    ) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            agent_id: agent_id.into(),
            priority: priority.clamp(1, 10),
            resource_requirements,
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            deadline: None,
            node_preference: None,
        }
    }

    /// Derived priority classification.
    #[must_use]
    pub fn priority_class(&self) -> TaskPriority {
        TaskPriority::from_numeric(self.priority)
    }

    /// Transition to a new status, returning an error on invalid transition.
    pub fn transition(&mut self, to: TaskStatus) -> Result<()> {
        if !self.status.valid_transition(&to) {
            return Err(DaimonError::SchedulerError(format!(
                "invalid transition: {:?} -> {:?}",
                self.status, to
            )));
        }
        debug!(task_id = %self.task_id, from = ?self.status, to = ?to, "task transition");
        self.status = to;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// NodeCapacity
// ---------------------------------------------------------------------------

/// Resource capacity descriptor for a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeCapacity {
    /// Node identifier.
    pub node_id: String,
    /// Total CPU cores.
    pub total_cpu: f64,
    /// Available CPU cores.
    pub available_cpu: f64,
    /// Total memory in MB.
    pub total_memory_mb: u64,
    /// Available memory in MB.
    pub available_memory_mb: u64,
    /// Total disk in MB.
    pub total_disk_mb: u64,
    /// Available disk in MB.
    pub available_disk_mb: u64,
    /// Whether a GPU is available.
    pub gpu_available: bool,
    /// Whether a TPU is available.
    pub tpu_available: bool,
    /// Number of TPU chips.
    pub tpu_chip_count: u32,
    /// Number of currently running tasks.
    pub running_tasks: usize,
}

impl NodeCapacity {
    /// Create a new node capacity descriptor.
    pub fn new(
        node_id: impl Into<String>,
        total_cpu: f64,
        total_memory_mb: u64,
        total_disk_mb: u64,
        gpu_available: bool,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            total_cpu,
            available_cpu: total_cpu,
            total_memory_mb,
            available_memory_mb: total_memory_mb,
            total_disk_mb,
            available_disk_mb: total_disk_mb,
            gpu_available,
            tpu_available: false,
            tpu_chip_count: 0,
            running_tasks: 0,
        }
    }

    /// Configure TPU availability.
    #[must_use]
    pub fn with_tpu(mut self, chip_count: u32) -> Self {
        self.tpu_available = chip_count > 0;
        self.tpu_chip_count = chip_count;
        self
    }

    /// Whether this node can fit the given resource requirements.
    #[must_use]
    pub fn can_fit(&self, req: &ResourceReq) -> bool {
        self.available_cpu >= req.cpu_cores
            && self.available_memory_mb >= req.memory_mb
            && self.available_disk_mb >= req.disk_mb
            && match &req.accelerator {
                AcceleratorRequirement::None => true,
                AcceleratorRequirement::Gpu => self.gpu_available,
                AcceleratorRequirement::Tpu { min_chips } => {
                    self.tpu_available && self.tpu_chip_count >= *min_chips
                }
                AcceleratorRequirement::GpuOrTpu => self.gpu_available || self.tpu_available,
                AcceleratorRequirement::AnyAccelerator => self.gpu_available || self.tpu_available,
            }
    }

    /// Utilization as a ratio 0.0-1.0 (average of CPU and memory).
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.total_cpu == 0.0 && self.total_memory_mb == 0 {
            return 0.0;
        }
        let cpu_util = if self.total_cpu > 0.0 {
            1.0 - (self.available_cpu / self.total_cpu)
        } else {
            0.0
        };
        let mem_util = if self.total_memory_mb > 0 {
            1.0 - (self.available_memory_mb as f64 / self.total_memory_mb as f64)
        } else {
            0.0
        };
        (cpu_util + mem_util) / 2.0
    }

    /// Reserve resources for a task.
    pub fn reserve(&mut self, req: &ResourceReq) {
        self.available_cpu = (self.available_cpu - req.cpu_cores).max(0.0);
        self.available_memory_mb = self.available_memory_mb.saturating_sub(req.memory_mb);
        self.available_disk_mb = self.available_disk_mb.saturating_sub(req.disk_mb);
        self.running_tasks += 1;
    }

    /// Release resources after a task completes.
    pub fn release(&mut self, req: &ResourceReq) {
        self.available_cpu = (self.available_cpu + req.cpu_cores).min(self.total_cpu);
        self.available_memory_mb =
            (self.available_memory_mb + req.memory_mb).min(self.total_memory_mb);
        self.available_disk_mb = (self.available_disk_mb + req.disk_mb).min(self.total_disk_mb);
        self.running_tasks = self.running_tasks.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------
// Scheduling decisions
// ---------------------------------------------------------------------------

/// A scheduler decision assigning a task to a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SchedulingDecision {
    /// The assigned task.
    pub task_id: String,
    /// The chosen node.
    pub assigned_node: String,
    /// Reason for this assignment.
    pub reason: String,
    /// Fitness score.
    pub score: f64,
}

/// A preemption action — one task evicts another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PreemptionAction {
    /// The task being evicted.
    pub preempted_task_id: String,
    /// The task that triggered eviction.
    pub preempting_task_id: String,
    /// Reason for preemption.
    pub reason: String,
}

/// Aggregate scheduler statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SchedulerStats {
    /// Total number of tasks tracked.
    pub total_tasks: usize,
    /// Tasks waiting in the queue.
    pub queued: usize,
    /// Tasks currently executing.
    pub running: usize,
    /// Tasks that completed (including cancelled).
    pub completed: usize,
    /// Tasks that failed.
    pub failed: usize,
    /// Average wait time in milliseconds.
    pub average_wait_time_ms: u64,
    /// Average run time in milliseconds.
    pub average_run_time_ms: u64,
}

// ---------------------------------------------------------------------------
// TaskScheduler
// ---------------------------------------------------------------------------

/// Core task scheduler.
pub struct TaskScheduler {
    tasks: HashMap<String, ScheduledTask>,
    nodes: HashMap<String, NodeCapacity>,
}

impl TaskScheduler {
    /// Create a new empty scheduler.
    pub fn new() -> Self {
        info!("task scheduler initialised");
        Self {
            tasks: HashMap::new(),
            nodes: HashMap::new(),
        }
    }

    /// Register a cluster node with its capacity.
    pub fn register_node(&mut self, node: NodeCapacity) {
        info!(node_id = %node.node_id, "registered node");
        self.nodes.insert(node.node_id.clone(), node);
    }

    /// Submit a new task. Returns the assigned task_id.
    pub fn submit_task(&mut self, task: ScheduledTask) -> Result<String> {
        let id = task.task_id.clone();
        info!(task_id = %id, name = %task.name, priority = task.priority, "task submitted");
        self.tasks.insert(id.clone(), task);
        Ok(id)
    }

    /// Get a task by ID.
    #[must_use]
    pub fn get_task(&self, task_id: &str) -> Option<&ScheduledTask> {
        self.tasks.get(task_id)
    }

    /// Get a mutable task by ID.
    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut ScheduledTask> {
        self.tasks.get_mut(task_id)
    }

    /// Cancel a task (valid from Queued, Scheduled, or Running).
    pub fn cancel_task(&mut self, task_id: &str) -> Result<()> {
        // Extract release info before mutably borrowing the task.
        let release_info = self.tasks.get(task_id).and_then(|t| {
            if matches!(t.status, TaskStatus::Running | TaskStatus::Scheduled) {
                t.node_preference
                    .clone()
                    .map(|node_id| (node_id, t.resource_requirements.clone()))
            } else {
                None
            }
        });

        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| DaimonError::SchedulerError(format!("task not found: {task_id}")))?;

        task.transition(TaskStatus::Cancelled)?;
        task.completed_at = Some(Utc::now());

        if let Some((node_id, req)) = release_info
            && let Some(node) = self.nodes.get_mut(&node_id)
        {
            node.release(&req);
        }
        info!(task_id = %task_id, "task cancelled");
        Ok(())
    }

    /// Pending (Queued) tasks sorted by priority desc, then created_at asc.
    #[must_use]
    pub fn pending_tasks(&self) -> Vec<&ScheduledTask> {
        let mut pending: Vec<&ScheduledTask> = self
            .tasks
            .values()
            .filter(|t| t.status == TaskStatus::Queued)
            .collect();
        pending.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });
        pending
    }

    /// Tasks assigned to a specific node.
    #[must_use]
    pub fn tasks_for_node(&self, node_id: &str) -> Vec<&ScheduledTask> {
        self.tasks
            .values()
            .filter(|t| t.node_preference.as_deref() == Some(node_id))
            .filter(|t| matches!(t.status, TaskStatus::Scheduled | TaskStatus::Running))
            .collect()
    }

    /// Schedule all pending tasks. Returns scheduling decisions.
    pub fn schedule_pending(&mut self) -> Vec<SchedulingDecision> {
        let mut decisions = Vec::new();
        let pending_ids: Vec<String> = {
            let mut pending: Vec<&ScheduledTask> = self
                .tasks
                .values()
                .filter(|t| t.status == TaskStatus::Queued)
                .collect();
            pending.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| a.created_at.cmp(&b.created_at))
            });
            pending.into_iter().map(|t| t.task_id.clone()).collect()
        };

        for task_id in pending_ids {
            let req = self.tasks[&task_id].resource_requirements.clone();
            let pref = self.tasks[&task_id].node_preference.clone();

            let chosen = if let Some(ref pref_id) = pref {
                if self.nodes.get(pref_id).is_some_and(|n| n.can_fit(&req)) {
                    Some(pref_id.clone())
                } else {
                    self.best_fit_node(&req)
                }
            } else {
                self.best_fit_node(&req)
            };

            if let Some(node_id) = chosen {
                let score = 1.0 - self.nodes[&node_id].utilization();

                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.reserve(&req);
                }

                let Some(task) = self.tasks.get_mut(&task_id) else {
                    continue;
                };
                let _ = task.transition(TaskStatus::Scheduled);
                task.scheduled_at = Some(Utc::now());
                task.node_preference = Some(node_id.clone());

                let reason = if pref.as_deref() == Some(&node_id) {
                    "preferred node".to_string()
                } else {
                    "best-fit selection".to_string()
                };

                debug!(task_id = %task_id, node = %node_id, score, "task scheduled");
                decisions.push(SchedulingDecision {
                    task_id,
                    assigned_node: node_id,
                    reason,
                    score,
                });
            } else {
                warn!(task_id = %task_id, "no node with sufficient capacity");
            }
        }

        decisions
    }

    /// Check if a high-priority task should preempt an existing running task.
    #[must_use]
    pub fn preempt_if_needed(&self, task: &ScheduledTask) -> Option<PreemptionAction> {
        let new_prio = task.priority_class();

        let mut candidate: Option<&ScheduledTask> = None;
        for t in self.tasks.values() {
            if t.status != TaskStatus::Running || !new_prio.can_preempt(&t.priority_class()) {
                continue;
            }
            match candidate {
                None => candidate = Some(t),
                Some(c) if t.priority < c.priority => candidate = Some(t),
                Some(c) if t.priority == c.priority && t.created_at > c.created_at => {
                    candidate = Some(t);
                }
                _ => {}
            }
        }

        candidate.map(|c| {
            info!(preempting = %task.task_id, preempted = %c.task_id, "preemption recommended");
            PreemptionAction {
                preempted_task_id: c.task_id.clone(),
                preempting_task_id: task.task_id.clone(),
                reason: format!(
                    "priority {} preempts priority {} task",
                    task.priority, c.priority
                ),
            }
        })
    }

    /// Compute aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> SchedulerStats {
        let mut stats = SchedulerStats {
            total_tasks: self.tasks.len(),
            ..Default::default()
        };

        let mut wait_times = Vec::new();
        let mut run_times = Vec::new();

        for t in self.tasks.values() {
            match &t.status {
                TaskStatus::Queued | TaskStatus::Scheduled | TaskStatus::Preempted => {
                    stats.queued += 1;
                }
                TaskStatus::Running => stats.running += 1,
                TaskStatus::Completed | TaskStatus::Cancelled => {
                    stats.completed += 1;
                    if let (Some(started), Some(completed)) = (t.started_at, t.completed_at) {
                        run_times.push((completed - started).num_milliseconds().max(0) as u64);
                    }
                }
                TaskStatus::Failed(_) => stats.failed += 1,
            }

            if let Some(started) = t.started_at {
                wait_times.push((started - t.created_at).num_milliseconds().max(0) as u64);
            }
        }

        if !wait_times.is_empty() {
            stats.average_wait_time_ms = wait_times.iter().sum::<u64>() / wait_times.len() as u64;
        }
        if !run_times.is_empty() {
            stats.average_run_time_ms = run_times.iter().sum::<u64>() / run_times.len() as u64;
        }

        stats
    }

    fn best_fit_node(&self, req: &ResourceReq) -> Option<String> {
        self.nodes
            .values()
            .filter(|n| n.can_fit(req))
            .min_by(|a, b| {
                a.utilization()
                    .partial_cmp(&b.utilization())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| n.node_id.clone())
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CronScheduler
// ---------------------------------------------------------------------------

/// A simplified cron-like schedule entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CronEntry {
    /// Entry name (unique key).
    pub name: String,
    /// Interval in seconds between fires.
    pub interval_seconds: u64,
    /// Optional specific hour (0-23).
    pub specific_hour: Option<u8>,
    /// Optional specific minute (0-59).
    pub specific_minute: Option<u8>,
    /// Template task to create when fired.
    pub task_template: CronTaskTemplate,
    /// Whether the entry is active.
    pub enabled: bool,
    /// Last time this entry fired.
    pub last_fired: Option<DateTime<Utc>>,
}

/// Template for creating tasks from cron entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CronTaskTemplate {
    /// Task name.
    pub name: String,
    /// Task description.
    pub description: String,
    /// Agent ID.
    pub agent_id: String,
    /// Numeric priority 1-10.
    pub priority: u8,
    /// Resource requirements.
    pub resource_requirements: ResourceReq,
}

/// Manages cron-like recurring task triggers.
pub struct CronScheduler {
    entries: HashMap<String, CronEntry>,
}

impl CronScheduler {
    /// Create a new cron scheduler.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a cron entry.
    pub fn add_entry(&mut self, entry: CronEntry) -> Result<()> {
        if entry.name.is_empty() {
            return Err(DaimonError::SchedulerError(
                "cron entry name cannot be empty".into(),
            ));
        }
        info!(name = %entry.name, interval = entry.interval_seconds, "cron entry added");
        self.entries.insert(entry.name.clone(), entry);
        Ok(())
    }

    /// Remove a cron entry by name.
    pub fn remove_entry(&mut self, name: &str) -> Result<()> {
        if self.entries.remove(name).is_none() {
            return Err(DaimonError::SchedulerError(format!(
                "cron entry not found: {name}"
            )));
        }
        info!(name = %name, "cron entry removed");
        Ok(())
    }

    /// List all entries.
    #[must_use]
    pub fn list_entries(&self) -> Vec<&CronEntry> {
        self.entries.values().collect()
    }

    /// Check which entries are due to fire. Updates `last_fired` for entries that fire.
    pub fn check_due(&mut self) -> Vec<ScheduledTask> {
        let now = Utc::now();
        let mut tasks = Vec::new();

        for entry in self.entries.values_mut() {
            if !entry.enabled {
                continue;
            }

            let should_fire = if let Some(last) = entry.last_fired {
                let elapsed = (now - last).num_seconds().max(0) as u64;
                elapsed >= entry.interval_seconds
            } else {
                true
            };

            let current_hour = now.time().hour() as u8;
            let current_minute = now.time().minute() as u8;
            let time_matches = match (entry.specific_hour, entry.specific_minute) {
                (Some(h), Some(m)) => current_hour == h && current_minute == m,
                (Some(h), None) => current_hour == h,
                (None, Some(m)) => current_minute == m,
                (None, None) => true,
            };

            if should_fire && time_matches {
                let tmpl = &entry.task_template;
                let task = ScheduledTask::new(
                    format!("{} (cron)", tmpl.name),
                    &tmpl.description,
                    &tmpl.agent_id,
                    tmpl.priority,
                    tmpl.resource_requirements.clone(),
                );
                debug!(cron = %entry.name, task_id = %task.task_id, "cron entry fired");
                tasks.push(task);
                entry.last_fired = Some(now);
            }
        }

        tasks
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TrainingJobTemplate
// ---------------------------------------------------------------------------

/// Training method for model fine-tuning jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TrainingMethod {
    /// Low-Rank Adaptation.
    LoRA,
    /// Quantized LoRA.
    QLoRA,
    /// Full parameter fine-tuning.
    FullFineTune,
    /// Direct Preference Optimization.
    DPO,
    /// Reinforcement Learning from Human Feedback.
    RLHF,
    /// Knowledge distillation.
    Distillation,
}

impl fmt::Display for TrainingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoRA => write!(f, "lora"),
            Self::QLoRA => write!(f, "qlora"),
            Self::FullFineTune => write!(f, "full"),
            Self::DPO => write!(f, "dpo"),
            Self::RLHF => write!(f, "rlhf"),
            Self::Distillation => write!(f, "distillation"),
        }
    }
}

impl TrainingMethod {
    /// Returns the preferred accelerator requirement.
    #[must_use]
    pub fn preferred_accelerator(&self) -> AcceleratorRequirement {
        match self {
            Self::LoRA | Self::QLoRA => AcceleratorRequirement::Gpu,
            Self::FullFineTune | Self::DPO | Self::RLHF | Self::Distillation => {
                AcceleratorRequirement::GpuOrTpu
            }
        }
    }
}

/// Template for creating training job tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TrainingJobTemplate {
    /// Model repository ID (e.g. "meta-llama/Llama-2-7b").
    pub model_id: String,
    /// Training method.
    pub method: TrainingMethod,
    /// Dataset path or identifier.
    pub dataset: String,
    /// Target node (prefer GPU-equipped).
    pub target_node: Option<String>,
    /// Maximum training duration in seconds (0 = no limit).
    pub max_duration_secs: u64,
    /// Checkpoint interval in seconds.
    pub checkpoint_interval_secs: u64,
}

impl TrainingJobTemplate {
    /// Create a scheduled task from this template.
    #[must_use]
    pub fn to_scheduled_task(&self, agent_id: &str) -> ScheduledTask {
        let mut task = ScheduledTask::new(
            format!("train-{}-{}", self.method, self.model_id),
            format!("Training job: {} on {}", self.method, self.model_id),
            agent_id,
            6, // High priority
            ResourceReq {
                cpu_cores: 2.0,
                memory_mb: 4096,
                disk_mb: 10_000,
                accelerator: self.method.preferred_accelerator(),
                network: false,
            },
        );
        task.deadline = if self.max_duration_secs > 0 {
            Some(Utc::now() + chrono::Duration::seconds(self.max_duration_secs as i64))
        } else {
            None
        };
        task.node_preference = self.target_node.clone();
        task
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_req(cpu: f64, mem: u64) -> ResourceReq {
        ResourceReq {
            cpu_cores: cpu,
            memory_mb: mem,
            ..Default::default()
        }
    }

    fn make_task(name: &str, priority: u8) -> ScheduledTask {
        ScheduledTask::new(name, "test task", "agent-1", priority, make_req(1.0, 256))
    }

    fn make_node(id: &str, cpu: f64, mem: u64) -> NodeCapacity {
        NodeCapacity::new(id, cpu, mem, 10240, false)
    }

    fn make_template() -> CronTaskTemplate {
        CronTaskTemplate {
            name: "cron-job".into(),
            description: "recurring".into(),
            agent_id: "agent-1".into(),
            priority: 5,
            resource_requirements: make_req(1.0, 128),
        }
    }

    // -- Task submission --

    #[test]
    fn submit_task() {
        let mut sched = TaskScheduler::new();
        let id = sched.submit_task(make_task("t1", 5)).unwrap();
        assert!(sched.get_task(&id).is_some());
    }

    #[test]
    fn submit_task_initial_status() {
        let mut sched = TaskScheduler::new();
        let id = sched.submit_task(make_task("t1", 5)).unwrap();
        assert_eq!(sched.get_task(&id).unwrap().status, TaskStatus::Queued);
    }

    #[test]
    fn submit_multiple_tasks() {
        let mut sched = TaskScheduler::new();
        let id1 = sched.submit_task(make_task("t1", 3)).unwrap();
        let id2 = sched.submit_task(make_task("t2", 7)).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(sched.stats().total_tasks, 2);
    }

    // -- Status transitions --

    #[test]
    fn valid_transitions() {
        let mut task = make_task("t", 5);
        assert!(task.transition(TaskStatus::Scheduled).is_ok());
        assert!(task.transition(TaskStatus::Running).is_ok());
        assert!(task.transition(TaskStatus::Completed).is_ok());
    }

    #[test]
    fn valid_transition_preempted_requeue() {
        let mut task = make_task("t", 5);
        task.transition(TaskStatus::Scheduled).unwrap();
        task.transition(TaskStatus::Running).unwrap();
        task.transition(TaskStatus::Preempted).unwrap();
        assert!(task.transition(TaskStatus::Queued).is_ok());
    }

    #[test]
    fn invalid_transition_queued_to_completed() {
        let mut task = make_task("t", 5);
        assert!(task.transition(TaskStatus::Completed).is_err());
    }

    #[test]
    fn invalid_transition_completed_to_running() {
        let mut task = make_task("t", 5);
        task.transition(TaskStatus::Scheduled).unwrap();
        task.transition(TaskStatus::Running).unwrap();
        task.transition(TaskStatus::Completed).unwrap();
        assert!(task.transition(TaskStatus::Running).is_err());
    }

    // -- Priority --

    #[test]
    fn priority_from_numeric() {
        assert_eq!(TaskPriority::from_numeric(1), TaskPriority::Normal);
        assert_eq!(TaskPriority::from_numeric(4), TaskPriority::High);
        assert_eq!(TaskPriority::from_numeric(7), TaskPriority::Critical);
        assert_eq!(TaskPriority::from_numeric(10), TaskPriority::Emergency);
    }

    #[test]
    fn priority_ordering() {
        assert!(TaskPriority::Emergency > TaskPriority::Critical);
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
    }

    #[test]
    fn priority_clamped() {
        assert_eq!(
            ScheduledTask::new("t", "d", "a", 15, ResourceReq::default()).priority,
            10
        );
        assert_eq!(
            ScheduledTask::new("t", "d", "a", 0, ResourceReq::default()).priority,
            1
        );
    }

    #[test]
    fn pending_sorted_by_priority() {
        let mut sched = TaskScheduler::new();
        let mut low = make_task("low", 2);
        low.created_at = Utc::now() - Duration::seconds(10);
        let mut high = make_task("high", 8);
        high.created_at = Utc::now() - Duration::seconds(5);
        sched.submit_task(low).unwrap();
        sched.submit_task(high).unwrap();

        let pending = sched.pending_tasks();
        assert_eq!(pending[0].name, "high");
        assert_eq!(pending[1].name, "low");
    }

    #[test]
    fn pending_same_priority_by_created() {
        let mut sched = TaskScheduler::new();
        let mut older = make_task("older", 5);
        older.created_at = Utc::now() - Duration::seconds(100);
        let mut newer = make_task("newer", 5);
        newer.created_at = Utc::now();
        sched.submit_task(newer).unwrap();
        sched.submit_task(older).unwrap();

        let pending = sched.pending_tasks();
        assert_eq!(pending[0].name, "older");
    }

    // -- Preemption --

    #[test]
    fn preempt_higher_over_lower() {
        let mut sched = TaskScheduler::new();
        let mut running = make_task("low", 2);
        running.status = TaskStatus::Running;
        sched.submit_task(running).unwrap();

        let emergency = make_task("emergency", 10);
        assert!(sched.preempt_if_needed(&emergency).is_some());
    }

    #[test]
    fn no_preempt_same_priority() {
        let mut sched = TaskScheduler::new();
        let mut running = make_task("existing", 5);
        running.status = TaskStatus::Running;
        sched.submit_task(running).unwrap();
        assert!(sched.preempt_if_needed(&make_task("same", 5)).is_none());
    }

    #[test]
    fn can_preempt_method() {
        assert!(TaskPriority::Emergency.can_preempt(&TaskPriority::Normal));
        assert!(!TaskPriority::Normal.can_preempt(&TaskPriority::Normal));
        assert!(!TaskPriority::High.can_preempt(&TaskPriority::Critical));
    }

    // -- Node capacity --

    #[test]
    fn node_can_fit() {
        let node = make_node("n1", 4.0, 8192);
        assert!(node.can_fit(&make_req(2.0, 4096)));
        assert!(!node.can_fit(&make_req(5.0, 1024)));
        assert!(!node.can_fit(&make_req(1.0, 16384)));
    }

    #[test]
    fn node_gpu_requirement() {
        let node = NodeCapacity::new("n1", 4.0, 8192, 10240, false);
        let req = ResourceReq {
            accelerator: AcceleratorRequirement::Gpu,
            ..make_req(1.0, 256)
        };
        assert!(!node.can_fit(&req));
        assert!(NodeCapacity::new("n2", 4.0, 8192, 10240, true).can_fit(&req));
    }

    #[test]
    fn node_tpu_requirement() {
        let node = NodeCapacity::new("n1", 4.0, 8192, 10240, false).with_tpu(4);
        let req = ResourceReq {
            accelerator: AcceleratorRequirement::Tpu { min_chips: 2 },
            ..make_req(1.0, 256)
        };
        assert!(node.can_fit(&req));
    }

    #[test]
    fn node_utilization() {
        let mut node = make_node("n1", 4.0, 1000);
        assert_eq!(node.utilization(), 0.0);
        node.reserve(&make_req(2.0, 500));
        assert!((node.utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn node_reserve_release() {
        let mut node = make_node("n1", 4.0, 2048);
        let req = make_req(2.0, 1024);
        node.reserve(&req);
        assert!((node.available_cpu - 2.0).abs() < 0.001);
        assert_eq!(node.running_tasks, 1);

        node.release(&req);
        assert!((node.available_cpu - 4.0).abs() < 0.001);
        assert_eq!(node.running_tasks, 0);
    }

    // -- Scheduling --

    #[test]
    fn schedule_assigns_to_node() {
        let mut sched = TaskScheduler::new();
        sched.register_node(make_node("n1", 4.0, 4096));
        sched.submit_task(make_task("t1", 5)).unwrap();

        let decisions = sched.schedule_pending();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].assigned_node, "n1");
    }

    #[test]
    fn schedule_prefers_node_preference() {
        let mut sched = TaskScheduler::new();
        sched.register_node(make_node("n1", 8.0, 8192));
        sched.register_node(make_node("n2", 8.0, 8192));

        let mut task = make_task("t1", 5);
        task.node_preference = Some("n2".into());
        sched.submit_task(task).unwrap();

        let decisions = sched.schedule_pending();
        assert_eq!(decisions[0].assigned_node, "n2");
        assert_eq!(decisions[0].reason, "preferred node");
    }

    #[test]
    fn schedule_fallback_when_preferred_full() {
        let mut sched = TaskScheduler::new();
        let mut n1 = make_node("n1", 1.0, 128);
        n1.available_cpu = 0.0;
        n1.available_memory_mb = 0;
        sched.register_node(n1);
        sched.register_node(make_node("n2", 4.0, 4096));

        let mut task = make_task("t1", 5);
        task.node_preference = Some("n1".into());
        sched.submit_task(task).unwrap();

        let decisions = sched.schedule_pending();
        assert_eq!(decisions[0].assigned_node, "n2");
    }

    #[test]
    fn schedule_no_nodes() {
        let mut sched = TaskScheduler::new();
        sched.submit_task(make_task("t1", 5)).unwrap();
        assert!(sched.schedule_pending().is_empty());
        assert_eq!(sched.pending_tasks().len(), 1);
    }

    // -- Cancel --

    #[test]
    fn cancel_queued_task() {
        let mut sched = TaskScheduler::new();
        let id = sched.submit_task(make_task("t1", 5)).unwrap();
        assert!(sched.cancel_task(&id).is_ok());
        assert_eq!(sched.get_task(&id).unwrap().status, TaskStatus::Cancelled);
    }

    #[test]
    fn cancel_nonexistent() {
        let mut sched = TaskScheduler::new();
        assert!(sched.cancel_task("nope").is_err());
    }

    // -- Stats --

    #[test]
    fn stats_empty() {
        let sched = TaskScheduler::new();
        let s = sched.stats();
        assert_eq!(s.total_tasks, 0);
    }

    #[test]
    fn stats_counts() {
        let mut sched = TaskScheduler::new();
        sched.submit_task(make_task("q1", 3)).unwrap();
        sched.submit_task(make_task("q2", 5)).unwrap();
        let mut running = make_task("r1", 7);
        running.status = TaskStatus::Running;
        sched.submit_task(running).unwrap();

        let s = sched.stats();
        assert_eq!(s.total_tasks, 3);
        assert_eq!(s.queued, 2);
        assert_eq!(s.running, 1);
    }

    #[test]
    fn tasks_for_node_filter() {
        let mut sched = TaskScheduler::new();
        sched.register_node(make_node("n1", 8.0, 8192));
        sched.register_node(make_node("n2", 8.0, 8192));

        let mut t1 = make_task("t1", 5);
        t1.node_preference = Some("n1".into());
        let mut t2 = make_task("t2", 5);
        t2.node_preference = Some("n2".into());
        sched.submit_task(t1).unwrap();
        sched.submit_task(t2).unwrap();
        sched.schedule_pending();

        assert_eq!(sched.tasks_for_node("n1").len(), 1);
        assert_eq!(sched.tasks_for_node("n2").len(), 1);
        assert_eq!(sched.tasks_for_node("n3").len(), 0);
    }

    // -- Cron --

    #[test]
    fn cron_add_entry() {
        let mut cron = CronScheduler::new();
        let entry = CronEntry {
            name: "backup".into(),
            interval_seconds: 3600,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: None,
        };
        assert!(cron.add_entry(entry).is_ok());
        assert_eq!(cron.list_entries().len(), 1);
    }

    #[test]
    fn cron_empty_name_rejected() {
        let mut cron = CronScheduler::new();
        let entry = CronEntry {
            name: "".into(),
            interval_seconds: 60,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: None,
        };
        assert!(cron.add_entry(entry).is_err());
    }

    #[test]
    fn cron_remove_entry() {
        let mut cron = CronScheduler::new();
        cron.add_entry(CronEntry {
            name: "backup".into(),
            interval_seconds: 3600,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: None,
        })
        .unwrap();
        assert!(cron.remove_entry("backup").is_ok());
        assert!(cron.list_entries().is_empty());
    }

    #[test]
    fn cron_fires_first_time() {
        let mut cron = CronScheduler::new();
        cron.add_entry(CronEntry {
            name: "first".into(),
            interval_seconds: 60,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: None,
        })
        .unwrap();
        let tasks = cron.check_due();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].name.contains("cron"));
    }

    #[test]
    fn cron_respects_interval() {
        let mut cron = CronScheduler::new();
        cron.add_entry(CronEntry {
            name: "recent".into(),
            interval_seconds: 3600,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: Some(Utc::now()),
        })
        .unwrap();
        assert!(cron.check_due().is_empty());
    }

    #[test]
    fn cron_disabled_skipped() {
        let mut cron = CronScheduler::new();
        cron.add_entry(CronEntry {
            name: "off".into(),
            interval_seconds: 1,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: false,
            last_fired: None,
        })
        .unwrap();
        assert!(cron.check_due().is_empty());
    }

    #[test]
    fn cron_fires_when_overdue() {
        let mut cron = CronScheduler::new();
        cron.add_entry(CronEntry {
            name: "overdue".into(),
            interval_seconds: 10,
            specific_hour: None,
            specific_minute: None,
            task_template: make_template(),
            enabled: true,
            last_fired: Some(Utc::now() - Duration::seconds(60)),
        })
        .unwrap();
        assert_eq!(cron.check_due().len(), 1);
    }

    // -- Training jobs --

    #[test]
    fn training_method_display() {
        assert_eq!(TrainingMethod::LoRA.to_string(), "lora");
        assert_eq!(TrainingMethod::QLoRA.to_string(), "qlora");
        assert_eq!(TrainingMethod::FullFineTune.to_string(), "full");
    }

    #[test]
    fn training_job_creates_task() {
        let job = TrainingJobTemplate {
            model_id: "meta-llama/Llama-2-7b".into(),
            method: TrainingMethod::LoRA,
            dataset: "/data/train.jsonl".into(),
            target_node: None,
            max_duration_secs: 3600,
            checkpoint_interval_secs: 300,
        };
        let task = job.to_scheduled_task("agent-123");
        assert!(task.name.contains("lora"));
        assert_eq!(task.priority, 6);
        assert!(task.deadline.is_some());
    }

    #[test]
    fn training_job_no_deadline_when_zero() {
        let job = TrainingJobTemplate {
            model_id: "m".into(),
            method: TrainingMethod::QLoRA,
            dataset: "d".into(),
            target_node: Some("gpu-1".into()),
            max_duration_secs: 0,
            checkpoint_interval_secs: 0,
        };
        let task = job.to_scheduled_task("a");
        assert!(task.deadline.is_none());
        assert_eq!(task.node_preference.unwrap(), "gpu-1");
    }

    // -- Serde roundtrips --

    #[test]
    fn task_status_serde_roundtrip() {
        for status in [
            TaskStatus::Queued,
            TaskStatus::Scheduled,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed("oops".into()),
            TaskStatus::Cancelled,
            TaskStatus::Preempted,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn resource_req_serde_roundtrip() {
        let req = ResourceReq::default();
        let json = serde_json::to_string(&req).unwrap();
        let back: ResourceReq = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cpu_cores, req.cpu_cores);
        assert_eq!(back.memory_mb, req.memory_mb);
    }

    #[test]
    fn scheduler_stats_serde_roundtrip() {
        let stats = SchedulerStats {
            total_tasks: 10,
            queued: 3,
            running: 2,
            completed: 4,
            failed: 1,
            ..Default::default()
        };
        let json = serde_json::to_string(&stats).unwrap();
        let back: SchedulerStats = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total_tasks, 10);
    }
}
