//! Heartbeat types and configuration
//!
//! Defines the data types used for worker heartbeat monitoring and lease management.

use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

use super::super::worker_pool::WorkerId;

/// Unique identifier for a lease
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeaseId(pub u64);

impl std::fmt::Display for LeaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lease-{}", self.0)
    }
}

/// Lease lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseLifecycle {
    /// Lease is active and valid
    Active,
    /// Lease has expired (time-based)
    Expired,
    /// Lease was revoked (stale worker replaced)
    Revoked,
    /// Lease was replaced by a newer one
    Replaced,
    /// Lease was released voluntarily
    Released,
}

impl LeaseLifecycle {
    /// Check if this is a terminal state (no more updates allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            LeaseLifecycle::Expired
                | LeaseLifecycle::Revoked
                | LeaseLifecycle::Replaced
                | LeaseLifecycle::Released
        )
    }

    /// Check if a new lease can be acquired for this task.
    pub fn allows_new_lease(&self) -> bool {
        self.is_terminal()
    }
}

impl std::fmt::Display for LeaseLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LeaseLifecycle::Active => write!(f, "active"),
            LeaseLifecycle::Expired => write!(f, "expired"),
            LeaseLifecycle::Revoked => write!(f, "revoked"),
            LeaseLifecycle::Replaced => write!(f, "replaced"),
            LeaseLifecycle::Released => write!(f, "released"),
        }
    }
}

/// Worker lifecycle state for deterministic transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerLifecycle {
    /// Worker is active and healthy
    Active,
    /// Worker is degraded (heartbeat late but not stale)
    Degraded,
    /// Worker is stale (heartbeat too late)
    Stale,
    /// Worker was replaced by another worker
    Replaced,
    /// Worker released ownership voluntarily
    Released,
    /// Worker is offline (never registered or unregistered)
    Offline,
}

impl WorkerLifecycle {
    /// Valid transitions from this state.
    pub fn valid_transitions(&self) -> &'static [WorkerLifecycle] {
        match self {
            WorkerLifecycle::Active => &[WorkerLifecycle::Degraded, WorkerLifecycle::Released],
            WorkerLifecycle::Degraded => &[
                WorkerLifecycle::Active,
                WorkerLifecycle::Stale,
                WorkerLifecycle::Released,
            ],
            WorkerLifecycle::Stale => &[
                WorkerLifecycle::Active,
                WorkerLifecycle::Replaced,
                WorkerLifecycle::Released,
            ],
            WorkerLifecycle::Replaced | WorkerLifecycle::Released | WorkerLifecycle::Offline => &[],
        }
    }

    /// Check if transition to target state is valid.
    pub fn can_transition_to(&self, target: WorkerLifecycle) -> bool {
        self.valid_transitions().contains(&target)
    }

    /// Check if worker is still operational.
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            WorkerLifecycle::Active | WorkerLifecycle::Degraded | WorkerLifecycle::Stale
        )
    }

    /// Check if worker can be replaced.
    pub fn can_be_replaced(&self) -> bool {
        matches!(
            self,
            WorkerLifecycle::Stale | WorkerLifecycle::Released | WorkerLifecycle::Replaced
        )
    }
}

impl std::fmt::Display for WorkerLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerLifecycle::Active => write!(f, "active"),
            WorkerLifecycle::Degraded => write!(f, "degraded"),
            WorkerLifecycle::Stale => write!(f, "stale"),
            WorkerLifecycle::Replaced => write!(f, "replaced"),
            WorkerLifecycle::Released => write!(f, "released"),
            WorkerLifecycle::Offline => write!(f, "offline"),
        }
    }
}

/// Heartbeat data from a worker
#[derive(Debug, Clone)]
pub struct Heartbeat {
    /// Worker ID sending the heartbeat
    pub worker_id: WorkerId,
    /// Lease ID if worker is executing a task
    pub lease_id: Option<LeaseId>,
    /// Current task ID if any
    pub task_id: Option<String>,
    /// Timestamp of the heartbeat
    pub timestamp: Instant,
    /// Optional progress percentage (0-100)
    pub progress: Option<u8>,
    /// Optional status message
    pub message: Option<String>,
    /// Current phase if executing
    pub phase: Option<String>,
    /// Tokens used so far (if available)
    pub tokens_used: Option<u64>,
    /// Generation/epoch of the worker (for ownership tracking)
    pub generation: Option<u64>,
}

impl Heartbeat {
    /// Create a new heartbeat for a worker
    pub fn new(worker_id: WorkerId) -> Self {
        Self {
            worker_id,
            lease_id: None,
            task_id: None,
            timestamp: Instant::now(),
            progress: None,
            message: None,
            phase: None,
            tokens_used: None,
            generation: None,
        }
    }

    /// Add lease information
    pub fn with_lease(mut self, lease_id: LeaseId, task_id: impl Into<String>) -> Self {
        self.lease_id = Some(lease_id);
        self.task_id = Some(task_id.into());
        self
    }

    /// Add progress information
    pub fn with_progress(mut self, progress: u8, message: impl Into<String>) -> Self {
        self.progress = Some(progress.min(100));
        self.message = Some(message.into());
        self
    }

    /// Add phase information
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    /// Add token usage
    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.tokens_used = Some(tokens);
        self
    }

    /// Add generation for ownership tracking
    pub fn with_generation(mut self, generation: u64) -> Self {
        self.generation = Some(generation);
        self
    }
}

/// Worker health status (for backward compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerHealth {
    /// Worker is healthy and sending regular heartbeats
    Healthy,
    /// Worker hasn't sent heartbeat recently (warning)
    Degraded,
    /// Worker is stale (no heartbeat for too long)
    Stale,
    /// Worker is offline
    Offline,
}

impl WorkerHealth {
    /// Convert to lifecycle state.
    pub fn to_lifecycle(&self) -> WorkerLifecycle {
        match self {
            WorkerHealth::Healthy => WorkerLifecycle::Active,
            WorkerHealth::Degraded => WorkerLifecycle::Degraded,
            WorkerHealth::Stale => WorkerLifecycle::Stale,
            WorkerHealth::Offline => WorkerLifecycle::Offline,
        }
    }
}

impl From<WorkerLifecycle> for WorkerHealth {
    fn from(lifecycle: WorkerLifecycle) -> Self {
        match lifecycle {
            WorkerLifecycle::Active => WorkerHealth::Healthy,
            WorkerLifecycle::Degraded => WorkerHealth::Degraded,
            WorkerLifecycle::Stale => WorkerHealth::Stale,
            WorkerLifecycle::Replaced | WorkerLifecycle::Released | WorkerLifecycle::Offline => {
                WorkerHealth::Offline
            }
        }
    }
}

impl std::fmt::Display for WorkerHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerHealth::Healthy => write!(f, "Healthy"),
            WorkerHealth::Degraded => write!(f, "Degraded"),
            WorkerHealth::Stale => write!(f, "Stale"),
            WorkerHealth::Offline => write!(f, "Offline"),
        }
    }
}

/// Configuration for heartbeat management
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Expected heartbeat interval
    pub heartbeat_interval: Duration,
    /// Time after which worker is considered degraded (no recent heartbeat)
    pub degraded_timeout: Duration,
    /// Time after which worker is considered stale
    pub stale_timeout: Duration,
    /// Time after which a lease is considered expired
    pub lease_expiry: Duration,
    /// Enable automatic lease expiration handling
    pub auto_expire_leases: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            degraded_timeout: Duration::from_secs(60),
            stale_timeout: Duration::from_secs(120),
            lease_expiry: Duration::from_secs(3600), // 1 hour
            auto_expire_leases: true,
        }
    }
}

/// Record of worker heartbeat state with lifecycle tracking
#[derive(Debug, Clone)]
pub struct WorkerHeartbeatState {
    /// Worker ID
    pub worker_id: WorkerId,
    /// Last heartbeat received
    pub last_heartbeat: Instant,
    /// Current health status (for backward compatibility)
    pub health: WorkerHealth,
    /// Lifecycle state for deterministic transitions
    pub lifecycle: WorkerLifecycle,
    /// Active lease ID if any
    pub active_lease: Option<LeaseId>,
    /// Current task ID if any
    pub current_task: Option<String>,
    /// Last reported progress
    pub last_progress: Option<u8>,
    /// Last status message
    pub last_message: Option<String>,
    /// Current phase
    pub current_phase: Option<String>,
    /// Total tokens used
    pub total_tokens: u64,
    /// Generation/epoch for ownership tracking
    pub generation: u64,
    /// When worker was registered
    pub registered_at: DateTime<Utc>,
    /// When lifecycle state last changed
    pub lifecycle_changed_at: Option<DateTime<Utc>>,
    /// Previous owner ID if replaced
    pub replaced_by: Option<WorkerId>,
}

impl WorkerHeartbeatState {
    pub(super) fn new(worker_id: WorkerId) -> Self {
        let now = Utc::now();
        Self {
            worker_id,
            last_heartbeat: Instant::now(),
            health: WorkerHealth::Healthy,
            lifecycle: WorkerLifecycle::Active,
            active_lease: None,
            current_task: None,
            last_progress: None,
            last_message: None,
            current_phase: None,
            total_tokens: 0,
            generation: 1,
            registered_at: now,
            lifecycle_changed_at: Some(now),
            replaced_by: None,
        }
    }

    /// Update from a heartbeat
    pub(super) fn update(&mut self, heartbeat: &Heartbeat) {
        self.last_heartbeat = heartbeat.timestamp;
        self.active_lease = heartbeat.lease_id;
        self.current_task = heartbeat.task_id.clone();
        self.last_progress = heartbeat.progress;
        self.last_message = heartbeat.message.clone();
        self.current_phase = heartbeat.phase.clone();
        if let Some(tokens) = heartbeat.tokens_used {
            self.total_tokens += tokens;
        }
        if let Some(gen) = heartbeat.generation {
            self.generation = gen;
        }
    }

    /// Transition to a new lifecycle state
    pub(super) fn transition_to(&mut self, new_lifecycle: WorkerLifecycle) -> bool {
        if self.lifecycle.can_transition_to(new_lifecycle) {
            self.lifecycle = new_lifecycle;
            self.lifecycle_changed_at = Some(Utc::now());
            self.health = new_lifecycle.into();
            true
        } else {
            false
        }
    }

    /// Mark worker as replaced by another worker
    pub(super) fn mark_replaced(&mut self, new_owner: WorkerId) {
        self.replaced_by = Some(new_owner);
        self.transition_to(WorkerLifecycle::Replaced);
    }

    /// Check health based on last heartbeat time (for backward compatibility)
    pub(super) fn check_health(&self, config: &HeartbeatConfig) -> WorkerHealth {
        let elapsed = self.last_heartbeat.elapsed();
        if elapsed > config.stale_timeout {
            WorkerHealth::Stale
        } else if elapsed > config.degraded_timeout {
            WorkerHealth::Degraded
        } else {
            WorkerHealth::Healthy
        }
    }
}

/// Lease state tracking with lifecycle
#[derive(Debug, Clone)]
pub struct LeaseState {
    /// Lease ID
    pub id: LeaseId,
    /// Worker ID holding the lease
    pub worker_id: WorkerId,
    /// Task ID the lease is for
    pub task_id: String,
    /// Lifecycle state of the lease
    pub lifecycle: LeaseLifecycle,
    /// When the lease was acquired
    pub acquired_at: Instant,
    /// When the lease was acquired (wall clock)
    pub acquired_at_wall: DateTime<Utc>,
    /// Maximum lease duration
    pub max_duration: Duration,
    /// Last renewal time
    pub last_renewed: Instant,
    /// Number of renewals
    pub renew_count: u32,
    /// Generation/epoch of this lease (for ownership)
    pub generation: u64,
    /// Reason for terminal state (if any)
    pub terminal_reason: Option<String>,
}

impl LeaseState {
    /// Create a new lease state
    pub fn new(
        id: LeaseId,
        worker_id: WorkerId,
        task_id: String,
        max_duration: Duration,
        generation: u64,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            worker_id,
            task_id,
            lifecycle: LeaseLifecycle::Active,
            acquired_at: now,
            acquired_at_wall: Utc::now(),
            max_duration,
            last_renewed: now,
            renew_count: 0,
            generation,
            terminal_reason: None,
        }
    }

    /// Check if the lease has expired
    pub fn is_expired(&self) -> bool {
        self.acquired_at.elapsed() > self.max_duration
    }

    /// Check if lease is in a terminal state
    pub fn is_terminal(&self) -> bool {
        self.lifecycle.is_terminal()
    }

    /// Get remaining time on the lease
    pub fn remaining_time(&self) -> Duration {
        self.max_duration.saturating_sub(self.acquired_at.elapsed())
    }

    /// Get elapsed time since lease was acquired
    pub fn elapsed(&self) -> Duration {
        self.acquired_at.elapsed()
    }

    /// Check if lease needs renewal
    pub fn needs_renewal(&self, threshold: Duration) -> bool {
        !self.is_terminal() && self.remaining_time() < threshold
    }

    /// Mark lease as expired
    pub fn mark_expired(&mut self) {
        self.lifecycle = LeaseLifecycle::Expired;
        self.terminal_reason = Some("Lease duration exceeded".to_string());
    }

    /// Mark lease as revoked (stale worker replaced)
    pub fn mark_revoked(&mut self, reason: &str) {
        self.lifecycle = LeaseLifecycle::Revoked;
        self.terminal_reason = Some(reason.to_string());
    }

    /// Mark lease as replaced
    pub fn mark_replaced(&mut self) {
        self.lifecycle = LeaseLifecycle::Replaced;
        self.terminal_reason = Some("Replaced by newer lease".to_string());
    }

    /// Mark lease as released voluntarily
    pub fn mark_released(&mut self) {
        self.lifecycle = LeaseLifecycle::Released;
        self.terminal_reason = Some("Released by owner".to_string());
    }
}

/// Statistics about heartbeat and lease state
#[derive(Debug, Clone, Default)]
pub struct HeartbeatStats {
    /// Total workers being tracked
    pub total_workers: usize,
    /// Healthy workers
    pub healthy_workers: usize,
    /// Degraded workers
    pub degraded_workers: usize,
    /// Stale workers
    pub stale_workers: usize,
    /// Replaced workers
    pub replaced_workers: usize,
    /// Active leases
    pub active_leases: usize,
    /// Expired leases
    pub expired_leases: usize,
    /// Revoked leases
    pub revoked_leases: usize,
    /// Total tokens consumed
    pub total_tokens: u64,
}

/// Result of a stale worker detection
#[derive(Debug, Clone)]
pub struct StaleWorkerInfo {
    /// Worker ID
    pub worker_id: WorkerId,
    /// Task ID that was being executed
    pub task_id: Option<String>,
    /// Lease ID if active
    pub lease_id: Option<LeaseId>,
    /// Current lifecycle state
    pub lifecycle: WorkerLifecycle,
    /// Time since last heartbeat
    pub last_heartbeat_ago: Duration,
    /// Generation/epoch
    pub generation: u64,
}

/// Result of a lease operation
#[derive(Debug, Clone)]
pub struct LeaseResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// The lease state after operation
    pub lease: Option<LeaseState>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Errors in heartbeat management
#[derive(Debug, thiserror::Error)]
pub enum HeartbeatError {
    /// Lease not found
    #[error("Lease not found: {0}")]
    LeaseNotFound(LeaseId),

    /// Lease has expired
    #[error("Lease has expired: {0}")]
    LeaseExpired(LeaseId),

    /// Lease is in terminal state
    #[error("Lease {0} is in terminal state: {1}")]
    LeaseTerminal(LeaseId, LeaseLifecycle),

    /// Worker not found
    #[error("Worker not found: {0}")]
    WorkerNotFound(WorkerId),

    /// Worker is in terminal state
    #[error("Worker {0} is in terminal state: {1}")]
    WorkerTerminal(WorkerId, WorkerLifecycle),

    /// Invalid heartbeat
    #[error("Invalid heartbeat: {0}")]
    InvalidHeartbeat(String),

    /// Generation mismatch (stale worker)
    #[error("Generation mismatch: expected {expected}, got {actual}")]
    GenerationMismatch { expected: u64, actual: u64 },
}
