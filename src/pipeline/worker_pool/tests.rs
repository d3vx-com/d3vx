use super::pool::WorkerPool;
use super::types::{WorkerId, WorkerPoolConfig, WorkerPoolError, WorkerStatus};
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_worker() {
        let pool = WorkerPool::with_defaults();
        let id = pool.add_worker("test-worker").await.unwrap();

        let worker = pool.get_worker(id).await.unwrap();
        assert_eq!(worker.name, "test-worker");
        assert_eq!(worker.status, WorkerStatus::Available);
    }

    #[tokio::test]
    async fn test_acquire_release_worker() {
        let pool = WorkerPool::with_defaults();
        pool.add_worker("test-worker").await.unwrap();

        // Acquire
        let lease = pool.acquire_worker("TASK-001").await.unwrap();
        assert_eq!(lease.task_id, "TASK-001");

        // Worker should be busy (capacity 1)
        let stats = pool.stats().await;
        assert_eq!(stats.busy_workers, 1);
        assert_eq!(stats.available_workers, 0);

        // Release
        pool.release_worker(lease).await.unwrap();

        let stats = pool.stats().await;
        assert_eq!(stats.available_workers, 1);
    }

    #[tokio::test]
    async fn test_no_workers_available() {
        let pool = WorkerPool::with_defaults();
        pool.add_worker("test-worker").await.unwrap();

        // Acquire the only worker
        let lease = pool.acquire_worker("TASK-001").await.unwrap();

        // Try to acquire another
        let result = pool.acquire_worker("TASK-002").await;
        assert!(matches!(result, Err(WorkerPoolError::NoWorkersAvailable)));

        pool.release_worker(lease).await.unwrap();
    }

    #[tokio::test]
    async fn test_worker_with_capacity() {
        let config = WorkerPoolConfig {
            default_capacity: 2,
            ..Default::default()
        };
        let pool = WorkerPool::new(config);
        pool.add_worker("multi-worker").await.unwrap();

        // Acquire first task
        let lease1 = pool.acquire_worker("TASK-001").await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.available_workers, 1); // Still available

        // Acquire second task
        let lease2 = pool.acquire_worker("TASK-002").await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.busy_workers, 1); // Now busy

        // Third should fail
        let result = pool.acquire_worker("TASK-003").await;
        assert!(matches!(result, Err(WorkerPoolError::NoWorkersAvailable)));

        pool.release_worker(lease1).await.unwrap();
        pool.release_worker(lease2).await.unwrap();
    }

    #[tokio::test]
    async fn test_pause_resume_worker() {
        let pool = WorkerPool::with_defaults();
        let id = pool.add_worker("test-worker").await.unwrap();

        // Pause
        pool.pause_worker(id).await.unwrap();
        let worker = pool.get_worker(id).await.unwrap();
        assert_eq!(worker.status, WorkerStatus::Paused);

        // Cannot acquire paused worker
        let result = pool.acquire_worker("TASK-001").await;
        assert!(matches!(result, Err(WorkerPoolError::NoWorkersAvailable)));

        // Resume
        pool.resume_worker(id).await.unwrap();
        let worker = pool.get_worker(id).await.unwrap();
        assert_eq!(worker.status, WorkerStatus::Available);

        // Now can acquire
        let lease = pool.acquire_worker("TASK-001").await;
        assert!(lease.is_ok());
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let pool = WorkerPool::with_defaults();
        pool.add_worker("worker-1").await.unwrap();
        pool.add_worker("worker-2").await.unwrap();

        let stats = pool.stats().await;
        assert_eq!(stats.total_workers, 2);
        assert_eq!(stats.available_workers, 2);

        let lease = pool.acquire_worker("TASK-001").await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.busy_workers, 1);
        assert_eq!(stats.available_workers, 1);
        assert_eq!(stats.active_leases, 1);

        pool.release_worker(lease).await.unwrap();
    }
}
