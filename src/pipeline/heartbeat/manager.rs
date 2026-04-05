//! Heartbeat manager implementation
//!
//! Provides the `HeartbeatManager` struct for tracking worker health and lease lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use super::super::ownership::{OwnerId, OwnershipManager};
use super::super::worker_pool::WorkerId;
use super::types::*;

/// The heartbeat manager
pub struct HeartbeatManager {
    /// Configuration
    pub(super) config: HeartbeatConfig,
    /// Worker heartbeat states
    worker_states: RwLock<HashMap<WorkerId, WorkerHeartbeatState>>,
    /// Active leases
    leases: Mutex<HashMap<LeaseId, LeaseState>>,
    /// Leases by task ID for quick lookup
    leases_by_task: Mutex<HashMap<String, LeaseId>>,
    /// Next lease ID
    next_lease_id: std::sync::atomic::AtomicU64,
    /// Optional ownership manager for task ownership tracking
    ownership_manager: Option<Arc<OwnershipManager>>,
}

impl HeartbeatManager {
    /// Create a new heartbeat manager
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            worker_states: RwLock::new(HashMap::new()),
            leases: Mutex::new(HashMap::new()),
            leases_by_task: Mutex::new(HashMap::new()),
            next_lease_id: std::sync::atomic::AtomicU64::new(1),
            ownership_manager: None,
        }
    }

    /// Create with optional ownership manager for task ownership tracking
    pub fn with_ownership(config: HeartbeatConfig, ownership: Arc<OwnershipManager>) -> Self {
        Self {
            config,
            worker_states: RwLock::new(HashMap::new()),
            leases: Mutex::new(HashMap::new()),
            leases_by_task: Mutex::new(HashMap::new()),
            next_lease_id: std::sync::atomic::AtomicU64::new(1),
            ownership_manager: Some(ownership),
        }
    }

    /// Get the ownership manager if configured
    pub fn ownership_manager(&self) -> Option<&Arc<OwnershipManager>> {
        self.ownership_manager.as_ref()
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(HeartbeatConfig::default())
    }

    /// Register a worker for heartbeat tracking
    pub async fn register_worker(&self, worker_id: WorkerId) {
        let mut states = self.worker_states.write().await;
        states.insert(worker_id, WorkerHeartbeatState::new(worker_id));
        info!("Registered worker {} for heartbeat tracking", worker_id);
    }

    /// Unregister a worker (marks as Released)
    pub async fn unregister_worker(&self, worker_id: WorkerId) {
        let mut states = self.worker_states.write().await;
        if let Some(state) = states.get_mut(&worker_id) {
            state.transition_to(WorkerLifecycle::Released);
        }
        info!("Unregistered worker {} from heartbeat tracking", worker_id);
    }

    /// Process a heartbeat from a worker
    pub async fn process_heartbeat(&self, heartbeat: Heartbeat) -> Result<(), HeartbeatError> {
        let mut states = self.worker_states.write().await;

        let state = states
            .entry(heartbeat.worker_id)
            .or_insert_with(|| WorkerHeartbeatState::new(heartbeat.worker_id));

        // Check if worker is in a terminal state
        if !state.lifecycle.is_operational() {
            return Err(HeartbeatError::WorkerTerminal(
                heartbeat.worker_id,
                state.lifecycle,
            ));
        }

        // Check generation if provided
        if let Some(gen) = heartbeat.generation {
            if gen < state.generation {
                return Err(HeartbeatError::GenerationMismatch {
                    expected: state.generation,
                    actual: gen,
                });
            }
        }

        state.update(&heartbeat);

        // Update lifecycle based on heartbeat health
        let new_health = state.check_health(&self.config);
        let target_lifecycle = new_health.to_lifecycle();
        if target_lifecycle != state.lifecycle
            && state.lifecycle.can_transition_to(target_lifecycle)
        {
            state.transition_to(target_lifecycle);
        } else if new_health == WorkerHealth::Healthy
            && state.lifecycle == WorkerLifecycle::Degraded
        {
            // Recover from degraded
            state.transition_to(WorkerLifecycle::Active);
        }

        debug!(
            "Processed heartbeat from worker {} (task: {:?}, progress: {:?}%, lifecycle: {:?})",
            heartbeat.worker_id, heartbeat.task_id, heartbeat.progress, state.lifecycle
        );

        Ok(())
    }

    /// Create a new lease for a task (marks previous lease as replaced)
    ///
    /// If ownership_manager is configured, this also acquires task ownership.
    pub async fn create_lease(
        &self,
        worker_id: WorkerId,
        task_id: impl Into<String>,
    ) -> Result<LeaseState, HeartbeatError> {
        let task_id = task_id.into();

        // Check worker state
        {
            let states = self.worker_states.read().await;
            if let Some(state) = states.get(&worker_id) {
                if !state.lifecycle.is_operational() {
                    return Err(HeartbeatError::WorkerTerminal(worker_id, state.lifecycle));
                }
            }
        }

        // Check for existing lease on this task and revoke it
        let existing_lease = {
            let by_task = self.leases_by_task.lock().await;
            by_task.get(&task_id).cloned()
        };

        if let Some(existing_id) = existing_lease {
            self.revoke_lease_internal(existing_id, "Replaced by newer lease")
                .await;
        }

        let lease_id = LeaseId(
            self.next_lease_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        // Get generation from worker state
        let generation = {
            let states = self.worker_states.read().await;
            states.get(&worker_id).map(|s| s.generation).unwrap_or(1)
        };

        // Acquire ownership if manager is configured
        if let Some(ref ownership) = self.ownership_manager {
            let owner = OwnerId::worker(worker_id.0);
            let result = ownership
                .acquire(&task_id, owner, Some(lease_id.0), true)
                .await;
            if !result.success {
                warn!(
                    "Failed to acquire ownership for task {}: {:?}",
                    task_id, result.error
                );
            }
        }

        let lease = LeaseState::new(
            lease_id,
            worker_id,
            task_id.clone(),
            self.config.lease_expiry,
            generation,
        );

        {
            let mut leases = self.leases.lock().await;
            leases.insert(lease_id, lease.clone());
        }

        {
            let mut by_task = self.leases_by_task.lock().await;
            by_task.insert(task_id.clone(), lease_id);
        }

        {
            let mut states = self.worker_states.write().await;
            if let Some(state) = states.get_mut(&worker_id) {
                state.active_lease = Some(lease_id);
                state.current_task = Some(task_id.clone());
            }
        }

        info!(
            "Created lease {} for worker {} on task {} (gen: {})",
            lease_id, worker_id, task_id, generation
        );

        Ok(lease)
    }

    /// Internal helper to revoke a lease
    async fn revoke_lease_internal(&self, lease_id: LeaseId, reason: &str) {
        let mut leases = self.leases.lock().await;
        if let Some(lease) = leases.get_mut(&lease_id) {
            lease.mark_revoked(reason);
        }
    }

    /// Renew a lease
    pub async fn renew_lease(&self, lease_id: LeaseId) -> Result<LeaseState, HeartbeatError> {
        let mut leases = self.leases.lock().await;

        let lease = leases
            .get_mut(&lease_id)
            .ok_or(HeartbeatError::LeaseNotFound(lease_id))?;

        if lease.is_terminal() {
            return Err(HeartbeatError::LeaseTerminal(lease_id, lease.lifecycle));
        }

        if lease.is_expired() {
            lease.mark_expired();
            return Err(HeartbeatError::LeaseExpired(lease_id));
        }

        lease.last_renewed = std::time::Instant::now();
        lease.renew_count += 1;

        debug!(
            "Renewed lease {} (renewal #{})",
            lease_id, lease.renew_count
        );

        Ok(lease.clone())
    }

    /// Release a lease voluntarily
    pub async fn release_lease(&self, lease_id: LeaseId) -> LeaseResult {
        let lease = {
            let mut leases = self.leases.lock().await;
            match leases.get_mut(&lease_id) {
                Some(lease) if !lease.is_terminal() => {
                    lease.mark_released();
                    Some(lease.clone())
                }
                Some(lease) => {
                    return LeaseResult {
                        success: false,
                        lease: Some(lease.clone()),
                        error: Some(format!(
                            "Lease already in terminal state: {:?}",
                            lease.lifecycle
                        )),
                    }
                }
                None => {
                    return LeaseResult {
                        success: false,
                        lease: None,
                        error: Some(format!("Lease not found: {}", lease_id)),
                    }
                }
            }
        };

        if let Some(ref l) = lease {
            // Remove from by_task index
            {
                let mut by_task = self.leases_by_task.lock().await;
                by_task.remove(&l.task_id);
            }

            // Release ownership if manager is configured
            if let Some(ref ownership) = self.ownership_manager {
                let owner = OwnerId::worker(l.worker_id.0);
                let _ = ownership.release(&l.task_id, &owner).await;
            }

            // Clear worker state
            {
                let mut states = self.worker_states.write().await;
                if let Some(state) = states.get_mut(&l.worker_id) {
                    state.active_lease = None;
                    state.current_task = None;
                }
            }

            info!(
                "Released lease {} for worker {} on task {}",
                lease_id, l.worker_id, l.task_id
            );
        }

        LeaseResult {
            success: true,
            lease,
            error: None,
        }
    }

    /// Revoke a lease due to staleness (called by recovery system)
    pub async fn revoke_lease(&self, lease_id: LeaseId, reason: &str) -> LeaseResult {
        let mut leases = self.leases.lock().await;

        match leases.get_mut(&lease_id) {
            Some(lease) if !lease.is_terminal() => {
                lease.mark_revoked(reason);
                let lease_clone = lease.clone();

                // Remove from by_task index
                {
                    let mut by_task = self.leases_by_task.lock().await;
                    by_task.remove(&lease.task_id);
                }

                // Mark worker as replaced
                {
                    let mut states = self.worker_states.write().await;
                    if let Some(state) = states.get_mut(&lease.worker_id) {
                        state.mark_replaced(WorkerId(0)); // Placeholder
                        state.active_lease = None;
                        state.current_task = None;
                    }
                }

                info!(
                    "Revoked lease {} for worker {} on task {}: {}",
                    lease_id, lease.worker_id, lease.task_id, reason
                );

                LeaseResult {
                    success: true,
                    lease: Some(lease_clone),
                    error: None,
                }
            }
            Some(lease) => LeaseResult {
                success: false,
                lease: Some(lease.clone()),
                error: Some(format!(
                    "Lease already in terminal state: {:?}",
                    lease.lifecycle
                )),
            },
            None => LeaseResult {
                success: false,
                lease: None,
                error: Some(format!("Lease not found: {}", lease_id)),
            },
        }
    }

    /// Replace a stale worker with a new one
    pub async fn replace_worker(
        &self,
        stale_worker_id: WorkerId,
        new_worker_id: WorkerId,
    ) -> Result<(), HeartbeatError> {
        let mut states = self.worker_states.write().await;

        // Check if stale worker exists
        let stale_state = states
            .get(&stale_worker_id)
            .ok_or(HeartbeatError::WorkerNotFound(stale_worker_id))?;

        if !stale_state.lifecycle.can_be_replaced() {
            return Err(HeartbeatError::WorkerTerminal(
                stale_worker_id,
                stale_state.lifecycle,
            ));
        }

        // Mark stale worker as replaced
        if let Some(state) = states.get_mut(&stale_worker_id) {
            state.mark_replaced(new_worker_id);

            // Release any active lease
            if let Some(lease_id) = state.active_lease {
                drop(states); // Release read lock
                let _ = self.revoke_lease(lease_id, "Worker replaced").await;
                states = self.worker_states.write().await;
            }
        }

        // Register new worker
        states.insert(new_worker_id, WorkerHeartbeatState::new(new_worker_id));

        info!(
            "Replaced stale worker {} with new worker {}",
            stale_worker_id, new_worker_id
        );

        Ok(())
    }

    /// Get lease by ID
    pub async fn get_lease(&self, lease_id: LeaseId) -> Option<LeaseState> {
        let leases = self.leases.lock().await;
        leases.get(&lease_id).cloned()
    }

    /// Get lease by task ID
    pub async fn get_lease_by_task(&self, task_id: &str) -> Option<LeaseState> {
        let by_task = self.leases_by_task.lock().await;
        let lease_id = by_task.get(task_id)?;
        let leases = self.leases.lock().await;
        leases.get(lease_id).cloned()
    }

    /// Check worker health
    pub async fn check_worker_health(&self, worker_id: WorkerId) -> WorkerHealth {
        let states = self.worker_states.read().await;
        states
            .get(&worker_id)
            .map(|s| s.check_health(&self.config))
            .unwrap_or(WorkerHealth::Offline)
    }

    /// Get worker lifecycle state
    pub async fn get_worker_lifecycle(&self, worker_id: WorkerId) -> Option<WorkerLifecycle> {
        let states = self.worker_states.read().await;
        states.get(&worker_id).map(|s| s.lifecycle)
    }

    /// Detect stale workers with full info
    pub async fn detect_stale_workers(&self) -> Vec<StaleWorkerInfo> {
        let states = self.worker_states.read().await;
        let mut stale = Vec::new();

        for (worker_id, state) in states.iter() {
            let health = state.check_health(&self.config);
            if health == WorkerHealth::Stale || state.lifecycle == WorkerLifecycle::Stale {
                stale.push(StaleWorkerInfo {
                    worker_id: *worker_id,
                    task_id: state.current_task.clone(),
                    lease_id: state.active_lease,
                    lifecycle: state.lifecycle,
                    last_heartbeat_ago: state.last_heartbeat.elapsed(),
                    generation: state.generation,
                });
            }
        }

        if !stale.is_empty() {
            warn!("Detected {} stale workers", stale.len());
        }

        stale
    }

    /// Detect expired leases
    pub async fn detect_expired_leases(&self) -> Vec<LeaseState> {
        let mut leases = self.leases.lock().await;
        let mut expired = Vec::new();

        for lease in leases.values_mut() {
            if !lease.is_terminal() && lease.is_expired() {
                lease.mark_expired();
                expired.push(lease.clone());
            }
        }

        if !expired.is_empty() {
            warn!("Detected {} expired leases", expired.len());
        }

        expired
    }

    /// Get statistics
    pub async fn stats(&self) -> HeartbeatStats {
        let states = self.worker_states.read().await;
        let leases = self.leases.lock().await;

        let mut stats = HeartbeatStats {
            total_workers: states.len(),
            active_leases: leases.len(),
            ..Default::default()
        };

        for state in states.values() {
            stats.total_tokens += state.total_tokens;
            match state.lifecycle {
                WorkerLifecycle::Active => stats.healthy_workers += 1,
                WorkerLifecycle::Degraded => stats.degraded_workers += 1,
                WorkerLifecycle::Stale => stats.stale_workers += 1,
                WorkerLifecycle::Replaced => stats.replaced_workers += 1,
                WorkerLifecycle::Released | WorkerLifecycle::Offline => {}
            }
        }

        for lease in leases.values() {
            match lease.lifecycle {
                LeaseLifecycle::Expired => stats.expired_leases += 1,
                LeaseLifecycle::Revoked => stats.revoked_leases += 1,
                LeaseLifecycle::Active => {}
                LeaseLifecycle::Replaced | LeaseLifecycle::Released => {}
            }
        }

        stats
    }

    /// Get all worker states
    pub async fn get_all_worker_states(&self) -> Vec<WorkerHeartbeatState> {
        let states = self.worker_states.read().await;
        states.values().cloned().collect()
    }

    /// Get worker state
    pub async fn get_worker_state(&self, worker_id: WorkerId) -> Option<WorkerHeartbeatState> {
        let states = self.worker_states.read().await;
        states.get(&worker_id).cloned()
    }

    #[cfg(test)]
    pub(crate) async fn force_worker_lifecycle_for_test(
        &self,
        worker_id: WorkerId,
        lifecycle: WorkerLifecycle,
    ) -> bool {
        let mut states = self.worker_states.write().await;
        let Some(state) = states.get_mut(&worker_id) else {
            return false;
        };
        if state.lifecycle == lifecycle {
            return true;
        }
        if state.lifecycle.can_transition_to(lifecycle) {
            state.transition_to(lifecycle);
            return true;
        }
        false
    }

    /// Run periodic health check (call this from a background task)
    pub async fn run_health_check(&self) -> (Vec<StaleWorkerInfo>, Vec<LeaseState>) {
        let stale = self.detect_stale_workers().await;
        let expired = self.detect_expired_leases().await;
        (stale, expired)
    }

    /// Update health status for all workers
    pub async fn update_all_health(&self) {
        let mut states = self.worker_states.write().await;
        for state in states.values_mut() {
            state.health = state.check_health(&self.config);
            let target = state.health.to_lifecycle();
            if target != state.lifecycle && state.lifecycle.can_transition_to(target) {
                state.transition_to(target);
            }
        }
    }
}
