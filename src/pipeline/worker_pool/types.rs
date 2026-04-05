//! Worker Pool
//!
//! Manages available workers for task execution with lease semantics.

use std::time::{Duration, Instant};

/// Unique identifier for a worker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkerId(pub u64);

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker-{}", self.0)
    }
}

/// Status of a worker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    /// Worker is available for task assignment
    Available,
    /// Worker is actively executing a task
    Busy,
    /// Worker is paused (maintenance, cooldown, etc.)
    Paused,
    /// Worker is offline/unreachable
    Offline,
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatus::Available => write!(f, "Available"),
            WorkerStatus::Busy => write!(f, "Busy"),
            WorkerStatus::Paused => write!(f, "Paused"),
            WorkerStatus::Offline => write!(f, "Offline"),
        }
    }
}

/// A worker in the pool
#[derive(Debug, Clone)]
pub struct Worker {
    /// Unique identifier
    pub id: WorkerId,
    /// Current status
    pub status: WorkerStatus,
    /// Name/label for the worker
    pub name: String,
    /// Maximum concurrent tasks this worker can handle
    pub capacity: usize,
    /// Currently assigned tasks
    pub current_tasks: Vec<String>,
    /// Total tasks completed by this worker
    pub tasks_completed: u64,
    /// Last activity timestamp
    pub last_activity: Instant,
}

impl Worker {
    /// Create a new worker
    pub fn new(id: WorkerId, name: impl Into<String>) -> Self {
        Self {
            id,
            status: WorkerStatus::Available,
            name: name.into(),
            capacity: 1,
            current_tasks: Vec::new(),
            tasks_completed: 0,
            last_activity: Instant::now(),
        }
    }

    /// Create a worker with custom capacity
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Check if worker can accept more tasks
    pub fn can_accept_task(&self) -> bool {
        self.status == WorkerStatus::Available && self.current_tasks.len() < self.capacity
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.current_tasks.len())
    }

    /// Get utilization percentage (0-100)
    pub fn utilization(&self) -> u8 {
        if self.capacity == 0 {
            return 0;
        }
        ((self.current_tasks.len() * 100) / self.capacity) as u8
    }
}

/// A lease on a worker for task execution
#[derive(Debug)]
pub struct WorkerLease {
    /// Worker ID
    pub worker_id: WorkerId,
    /// Task ID this lease is for
    pub task_id: String,
    /// When the lease was acquired
    pub acquired_at: Instant,
    /// Maximum lease duration
    pub max_duration: Duration,
}

impl WorkerLease {
    /// Check if the lease has expired
    pub fn is_expired(&self) -> bool {
        self.acquired_at.elapsed() > self.max_duration
    }

    /// Get remaining time on the lease
    pub fn remaining_time(&self) -> Duration {
        self.max_duration.saturating_sub(self.acquired_at.elapsed())
    }

    /// Get elapsed time since lease was acquired
    pub fn elapsed(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Maximum number of workers in the pool
    pub max_workers: usize,
    /// Default worker capacity
    pub default_capacity: usize,
    /// Default lease duration
    pub default_lease_duration: Duration,
    /// Enable automatic lease expiration
    pub enable_auto_expiration: bool,
    /// Interval for checking expired leases
    pub expiration_check_interval: Duration,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            max_workers: 10,
            default_capacity: 1,
            default_lease_duration: Duration::from_secs(3600), // 1 hour
            enable_auto_expiration: true,
            expiration_check_interval: Duration::from_secs(60),
        }
    }
}

/// Statistics about the worker pool
#[derive(Debug, Clone, Default)]
pub struct WorkerPoolStats {
    /// Total workers
    pub total_workers: usize,
    /// Available workers
    pub available_workers: usize,
    /// Busy workers
    pub busy_workers: usize,
    /// Paused workers
    pub paused_workers: usize,
    /// Offline workers
    pub offline_workers: usize,
    /// Active leases
    pub active_leases: usize,
    /// Total tasks currently running
    pub tasks_running: usize,
    /// Total capacity
    pub total_capacity: usize,
    /// Used capacity
    pub used_capacity: usize,
}

/// Trait defining the worker pool interface (for dependency inversion)
#[async_trait::async_trait]
pub trait WorkerPoolManager: Send + Sync {
    /// Acquire a worker for a task
    async fn acquire(&self, task_id: &str) -> Result<WorkerLease, WorkerPoolError>;

    /// Release a worker lease
    async fn release(&self, lease: WorkerLease) -> Result<(), WorkerPoolError>;

    /// Get available worker count
    async fn available_count(&self) -> usize;

    /// Get pool statistics
    async fn stats(&self) -> WorkerPoolStats;
}

/// Errors that can occur in the worker pool
#[derive(Debug, thiserror::Error)]
pub enum WorkerPoolError {
    /// No workers available
    #[error("No workers available")]
    NoWorkersAvailable,

    /// Worker not found
    #[error("Worker not found: {0}")]
    WorkerNotFound(WorkerId),

    /// Invalid lease
    #[error("Invalid or expired lease for worker {0}")]
    InvalidLease(WorkerId),

    /// Worker at capacity
    #[error("Worker {0} is at capacity")]
    WorkerAtCapacity(WorkerId),

    /// Pool is at maximum capacity
    #[error("Worker pool is at maximum capacity ({0} workers)")]
    PoolAtCapacity(usize),

    /// Lease expired
    #[error("Lease for task {0} has expired")]
    LeaseExpired(String),
}
