//! Heartbeat and Lease Management - Tests

use super::super::worker_pool::WorkerId;
use super::manager::HeartbeatManager;
use super::types::*;

#[tokio::test]
async fn test_heartbeat_registration() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    let health = manager.check_worker_health(worker_id).await;
    assert_eq!(health, WorkerHealth::Healthy);

    let lifecycle = manager.get_worker_lifecycle(worker_id).await.unwrap();
    assert_eq!(lifecycle, WorkerLifecycle::Active);
}

#[tokio::test]
async fn test_lease_lifecycle_active() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();
    assert_eq!(lease.task_id, "TASK-001");
    assert!(!lease.is_expired());
    assert_eq!(lease.lifecycle, LeaseLifecycle::Active);
}

#[tokio::test]
async fn test_lease_release() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;
    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    let result = manager.release_lease(lease.id).await;
    assert!(result.success);

    // Lease should be in Released state
    let found = manager.get_lease(lease.id).await.unwrap();
    assert_eq!(found.lifecycle, LeaseLifecycle::Released);
}

#[tokio::test]
async fn test_lease_replace_newer() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;
    let lease1 = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    // Create new lease for same task - should replace old one
    let lease2 = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    // Old lease should be marked as Replaced
    let old_lease = manager.get_lease(lease1.id).await.unwrap();
    assert_eq!(old_lease.lifecycle, LeaseLifecycle::Replaced);

    // New lease should be Active
    let new_lease = manager.get_lease(lease2.id).await.unwrap();
    assert_eq!(new_lease.lifecycle, LeaseLifecycle::Active);
}

#[tokio::test]
async fn test_lease_renewal() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;
    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    let renewed = manager.renew_lease(lease.id).await.unwrap();
    assert_eq!(renewed.renew_count, 1);
}

#[tokio::test]
async fn test_lease_renewal_after_release_fails() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;
    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    manager.release_lease(lease.id).await;

    let result = manager.renew_lease(lease.id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_worker_lifecycle_transitions() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    let lifecycle = manager.get_worker_lifecycle(worker_id).await.unwrap();
    assert_eq!(lifecycle, WorkerLifecycle::Active);

    // Unregister marks as Released
    manager.unregister_worker(worker_id).await;

    let lifecycle = manager.get_worker_lifecycle(worker_id).await.unwrap();
    assert_eq!(lifecycle, WorkerLifecycle::Released);
}

#[tokio::test]
async fn test_worker_replacement() {
    let manager = HeartbeatManager::with_defaults();
    let stale_worker = WorkerId(1);
    let new_worker = WorkerId(2);

    manager.register_worker(stale_worker).await;

    // Create lease for stale worker
    let lease = manager
        .create_lease(stale_worker, "TASK-001")
        .await
        .unwrap();

    // Mark worker as degraded then stale so it can be replaced
    manager
        .force_worker_lifecycle_for_test(stale_worker, WorkerLifecycle::Degraded)
        .await;
    manager
        .force_worker_lifecycle_for_test(stale_worker, WorkerLifecycle::Stale)
        .await;

    // Replace stale worker with new one
    manager
        .replace_worker(stale_worker, new_worker)
        .await
        .unwrap();

    // Stale worker should be Replaced
    let stale_lifecycle = manager.get_worker_lifecycle(stale_worker).await.unwrap();
    assert_eq!(stale_lifecycle, WorkerLifecycle::Replaced);

    // New worker should be Active
    let new_lifecycle = manager.get_worker_lifecycle(new_worker).await.unwrap();
    assert_eq!(new_lifecycle, WorkerLifecycle::Active);

    // Old lease should be Revoked
    let revoked_lease = manager.get_lease(lease.id).await.unwrap();
    assert_eq!(revoked_lease.lifecycle, LeaseLifecycle::Revoked);
}

#[tokio::test]
async fn test_worker_generation_tracking() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    let state = manager.get_worker_state(worker_id).await.unwrap();
    assert_eq!(state.generation, 1);

    // Send heartbeat with generation
    let heartbeat = Heartbeat::new(worker_id).with_generation(2);
    manager.process_heartbeat(heartbeat).await.unwrap();

    let state = manager.get_worker_state(worker_id).await.unwrap();
    assert_eq!(state.generation, 2);
}

#[tokio::test]
async fn test_stale_worker_detection() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    // Create lease
    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    // Detect stale workers (should be none initially)
    let stale = manager.detect_stale_workers().await;
    assert!(stale.is_empty());

    // Manually mark as degraded then stale for testing.
    assert!(
        manager
            .force_worker_lifecycle_for_test(worker_id, WorkerLifecycle::Degraded)
            .await
    );
    assert!(
        manager
            .force_worker_lifecycle_for_test(worker_id, WorkerLifecycle::Stale)
            .await
    );

    let stale = manager.detect_stale_workers().await;
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].worker_id, worker_id);
    assert_eq!(stale[0].lifecycle, WorkerLifecycle::Stale);
    assert_eq!(stale[0].lease_id, Some(lease.id));
}

#[tokio::test]
async fn test_heartbeat_processing() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;

    let heartbeat = Heartbeat::new(worker_id)
        .with_progress(50, "Working on it")
        .with_phase("implement");

    manager.process_heartbeat(heartbeat).await.unwrap();

    let state = manager.get_worker_state(worker_id).await.unwrap();
    assert_eq!(state.last_progress, Some(50));
    assert_eq!(state.last_message, Some("Working on it".to_string()));
    assert_eq!(state.current_phase, Some("implement".to_string()));
}

#[tokio::test]
async fn test_heartbeat_rejected_from_replaced_worker() {
    let manager = HeartbeatManager::with_defaults();
    let stale_worker = WorkerId(1);
    let new_worker = WorkerId(2);

    manager.register_worker(stale_worker).await;

    // Mark as degraded then stale so it can be replaced
    manager
        .force_worker_lifecycle_for_test(stale_worker, WorkerLifecycle::Degraded)
        .await;
    manager
        .force_worker_lifecycle_for_test(stale_worker, WorkerLifecycle::Stale)
        .await;

    manager
        .replace_worker(stale_worker, new_worker)
        .await
        .unwrap();

    // Try to send heartbeat from replaced worker
    let heartbeat = Heartbeat::new(stale_worker).with_progress(75, "Still working");

    let result = manager.process_heartbeat(heartbeat).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_stats_include_lifecycle_states() {
    let manager = HeartbeatManager::with_defaults();

    manager.register_worker(WorkerId(1)).await;
    manager.register_worker(WorkerId(2)).await;

    // Mark worker 1 as degraded then stale so it can be replaced
    manager
        .force_worker_lifecycle_for_test(WorkerId(1), WorkerLifecycle::Degraded)
        .await;
    manager
        .force_worker_lifecycle_for_test(WorkerId(1), WorkerLifecycle::Stale)
        .await;

    // Replace one worker
    manager
        .replace_worker(WorkerId(1), WorkerId(3))
        .await
        .unwrap();

    let stats = manager.stats().await;
    assert_eq!(stats.total_workers, 3);
    assert_eq!(stats.healthy_workers, 2); // Worker 2 and 3
    assert_eq!(stats.replaced_workers, 1); // Worker 1
}

#[tokio::test]
async fn test_lease_revocation() {
    let manager = HeartbeatManager::with_defaults();
    let worker_id = WorkerId(1);

    manager.register_worker(worker_id).await;
    let lease = manager.create_lease(worker_id, "TASK-001").await.unwrap();

    let result = manager.revoke_lease(lease.id, "Worker became stale").await;
    assert!(result.success);

    let revoked = manager.get_lease(lease.id).await.unwrap();
    assert_eq!(revoked.lifecycle, LeaseLifecycle::Revoked);
    assert_eq!(
        revoked.terminal_reason,
        Some("Worker became stale".to_string())
    );
}

#[tokio::test]
async fn test_worker_lifecycle_state_validity() {
    assert!(WorkerLifecycle::Active.can_transition_to(WorkerLifecycle::Degraded));
    assert!(WorkerLifecycle::Active.can_transition_to(WorkerLifecycle::Released));
    assert!(!WorkerLifecycle::Active.can_transition_to(WorkerLifecycle::Replaced));
    assert!(WorkerLifecycle::Degraded.can_transition_to(WorkerLifecycle::Stale));
    assert!(WorkerLifecycle::Stale.can_transition_to(WorkerLifecycle::Replaced));
    assert!(!WorkerLifecycle::Replaced.can_transition_to(WorkerLifecycle::Active));
    assert!(!WorkerLifecycle::Released.can_transition_to(WorkerLifecycle::Active));
}

#[tokio::test]
async fn test_lease_lifecycle_state_validity() {
    assert!(LeaseLifecycle::Active.is_terminal() == false);
    assert!(LeaseLifecycle::Expired.is_terminal());
    assert!(LeaseLifecycle::Revoked.is_terminal());
    assert!(LeaseLifecycle::Replaced.is_terminal());
    assert!(LeaseLifecycle::Released.is_terminal());

    assert!(!LeaseLifecycle::Active.allows_new_lease());
    assert!(LeaseLifecycle::Expired.allows_new_lease());
}
