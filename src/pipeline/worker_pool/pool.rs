//! Worker pool implementation

use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

use super::types::{
    Worker, WorkerId, WorkerLease, WorkerPoolConfig, WorkerPoolError, WorkerPoolManager,
    WorkerPoolStats, WorkerStatus,
};
use tracing::{debug, info};
/// The worker pool managing available workers
pub struct WorkerPool {
    /// Configuration
    config: WorkerPoolConfig,
    /// All workers indexed by ID
    workers: RwLock<HashMap<WorkerId, Worker>>,
    /// Active leases: task_id -> worker_id
    leases: Mutex<HashMap<String, WorkerId>>,
    /// Next worker ID
    next_id: std::sync::atomic::AtomicU64,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(config: WorkerPoolConfig) -> Self {
        Self {
            config,
            workers: RwLock::new(HashMap::new()),
            leases: Mutex::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create a worker pool with default configuration
    pub fn with_defaults() -> Self {
        Self::new(WorkerPoolConfig::default())
    }

    /// Add a worker to the pool
    pub async fn add_worker(&self, name: impl Into<String>) -> Result<WorkerId, WorkerPoolError> {
        let mut workers = self.workers.write().await;

        // Check capacity
        if workers.len() >= self.config.max_workers {
            return Err(WorkerPoolError::PoolAtCapacity(self.config.max_workers));
        }

        // Generate ID
        let id = WorkerId(
            self.next_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        // Create worker
        let mut worker = Worker::new(id, name);
        worker.capacity = self.config.default_capacity;

        workers.insert(id, worker);
        info!("Added worker {} to pool", id);

        Ok(id)
    }

    /// Remove a worker from the pool
    pub async fn remove_worker(&self, id: WorkerId) -> Result<Worker, WorkerPoolError> {
        let mut workers = self.workers.write().await;
        workers
            .remove(&id)
            .ok_or(WorkerPoolError::WorkerNotFound(id))
    }

    /// Get a worker by ID
    pub async fn get_worker(&self, id: WorkerId) -> Option<Worker> {
        let workers = self.workers.read().await;
        workers.get(&id).cloned()
    }

    /// Acquire a worker for a task (returns a lease)
    pub async fn acquire_worker(&self, task_id: &str) -> Result<WorkerLease, WorkerPoolError> {
        info!("Acquiring worker for task {}", task_id);

        // Find an available worker
        let worker_id = {
            let workers = self.workers.read().await;
            let mut selected: Option<WorkerId> = None;

            for (id, worker) in workers.iter() {
                if worker.can_accept_task() {
                    selected = Some(*id);
                    break;
                }
            }

            selected.ok_or(WorkerPoolError::NoWorkersAvailable)?
        };

        // Update worker status
        {
            let mut workers = self.workers.write().await;
            let worker = workers
                .get_mut(&worker_id)
                .ok_or(WorkerPoolError::WorkerNotFound(worker_id))?;

            worker.current_tasks.push(task_id.to_string());
            worker.last_activity = Instant::now();

            // Update status if now at capacity
            if worker.current_tasks.len() >= worker.capacity {
                worker.status = WorkerStatus::Busy;
            }

            debug!(
                "Worker {} acquired for task {} (utilization: {}%)",
                worker_id,
                task_id,
                worker.utilization()
            );
        }

        // Create lease
        let lease = WorkerLease {
            worker_id,
            task_id: task_id.to_string(),
            acquired_at: Instant::now(),
            max_duration: self.config.default_lease_duration,
        };

        // Store lease (we need to track by task_id, so we store worker_id)
        {
            let mut leases = self.leases.lock().await;
            leases.insert(task_id.to_string(), worker_id);
        }

        Ok(lease)
    }

    /// Release a worker lease
    pub async fn release_worker(&self, lease: WorkerLease) -> Result<(), WorkerPoolError> {
        info!(
            "Releasing worker {} for task {}",
            lease.worker_id, lease.task_id
        );

        // Update worker status
        {
            let mut workers = self.workers.write().await;
            let worker = workers
                .get_mut(&lease.worker_id)
                .ok_or(WorkerPoolError::WorkerNotFound(lease.worker_id))?;

            // Remove task from current tasks
            worker.current_tasks.retain(|t| t != &lease.task_id);
            worker.tasks_completed += 1;
            worker.last_activity = Instant::now();

            // Update status if now has capacity
            if worker.current_tasks.len() < worker.capacity {
                worker.status = WorkerStatus::Available;
            }

            debug!(
                "Worker {} released for task {} (tasks completed: {})",
                lease.worker_id, lease.task_id, worker.tasks_completed
            );
        }

        // Remove lease
        {
            let mut leases = self.leases.lock().await;
            leases.remove(&lease.task_id);
        }

        Ok(())
    }

    /// Set worker status
    pub async fn set_worker_status(
        &self,
        id: WorkerId,
        status: WorkerStatus,
    ) -> Result<(), WorkerPoolError> {
        let mut workers = self.workers.write().await;
        let worker = workers
            .get_mut(&id)
            .ok_or(WorkerPoolError::WorkerNotFound(id))?;

        worker.status = status;
        worker.last_activity = Instant::now();

        info!("Worker {} status changed to {}", id, status);
        Ok(())
    }

    /// Get available worker count
    pub async fn available_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.values().filter(|w| w.can_accept_task()).count()
    }

    /// Get total worker count
    pub async fn total_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.len()
    }

    /// Get pool statistics
    pub async fn stats(&self) -> WorkerPoolStats {
        let workers = self.workers.read().await;
        let leases = self.leases.lock().await;

        let mut stats = WorkerPoolStats {
            total_workers: workers.len(),
            active_leases: leases.len(),
            ..Default::default()
        };

        for worker in workers.values() {
            stats.total_capacity += worker.capacity;
            stats.used_capacity += worker.current_tasks.len();
            stats.tasks_running += worker.current_tasks.len();

            match worker.status {
                WorkerStatus::Available => stats.available_workers += 1,
                WorkerStatus::Busy => stats.busy_workers += 1,
                WorkerStatus::Paused => stats.paused_workers += 1,
                WorkerStatus::Offline => stats.offline_workers += 1,
            }
        }

        stats
    }

    /// List all workers
    pub async fn list_workers(&self) -> Vec<Worker> {
        let workers = self.workers.read().await;
        workers.values().cloned().collect()
    }

    /// Pause a worker
    pub async fn pause_worker(&self, id: WorkerId) -> Result<(), WorkerPoolError> {
        self.set_worker_status(id, WorkerStatus::Paused).await
    }

    /// Resume a worker
    pub async fn resume_worker(&self, id: WorkerId) -> Result<(), WorkerPoolError> {
        let workers = self.workers.read().await;
        let worker = workers
            .get(&id)
            .ok_or(WorkerPoolError::WorkerNotFound(id))?;

        let new_status = if worker.current_tasks.is_empty() {
            WorkerStatus::Available
        } else {
            WorkerStatus::Busy
        };

        drop(workers);
        self.set_worker_status(id, new_status).await
    }

    /// Check and clean up expired leases
    pub async fn cleanup_expired_leases(&self) -> Vec<String> {
        // Note: Since we don't store the full lease anymore, we can't check expiration here
        // This would need to be tracked separately if needed
        Vec::new()
    }
}

#[async_trait::async_trait]
impl WorkerPoolManager for WorkerPool {
    async fn acquire(&self, task_id: &str) -> Result<WorkerLease, WorkerPoolError> {
        self.acquire_worker(task_id).await
    }

    async fn release(&self, lease: WorkerLease) -> Result<(), WorkerPoolError> {
        self.release_worker(lease).await
    }

    async fn available_count(&self) -> usize {
        self.available_count().await
    }

    async fn stats(&self) -> WorkerPoolStats {
        self.stats().await
    }
}
