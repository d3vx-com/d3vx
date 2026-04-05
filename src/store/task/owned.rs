//! Owned Task Store - Ownership-Enforced Task Mutations
//!
//! Wraps TaskStore with ownership validation to prevent stale workers
//! from mutating task state after ownership transfer.
//!
//! # Design
//!
//! This module provides an authoritative enforcement point for task mutations.
//! All task updates through this wrapper require valid ownership.

use crate::pipeline::ownership::{OwnerId, OwnerToken, OwnershipError, OwnershipManager};
use crate::store::database::DatabaseError;
use crate::store::task::{Task, TaskState, TaskStore, TaskUpdate};

/// Errors from owned task operations.
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum OwnedTaskError {
    #[error("Ownership error: {0}")]
    Ownership(#[from] OwnershipError),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Stale owner cannot mutate task: current generation is {current}, requestor has generation {stale}")]
    StaleOwner { stale: u64, current: u64 },

    #[error("Not the current owner of task")]
    NotOwner,
}

/// Result type for owned task operations.
#[allow(dead_code)]
pub type OwnedTaskResult<T> = Result<T, OwnedTaskError>;

/// Owned task store - TaskStore with ownership enforcement.
///
/// This wrapper ensures that task mutations are only accepted from
/// the current owner (as tracked by OwnershipManager).
#[allow(dead_code)]
pub struct OwnedTaskStore<'a> {
    inner: &'a TaskStore<'a>,
    ownership: &'a OwnershipManager,
}

#[allow(dead_code)]
impl<'a> OwnedTaskStore<'a> {
    /// Create a new owned task store.
    pub fn new(task_store: &'a TaskStore, ownership: &'a OwnershipManager) -> Self {
        Self {
            inner: task_store,
            ownership,
        }
    }

    /// Get a task by ID (no ownership required for reads).
    pub fn get(&self, id: &str) -> OwnedTaskResult<Option<Task>> {
        self.inner.get(id).map_err(OwnedTaskError::Database)
    }

    /// Transition task state with ownership validation.
    pub async fn transition(
        &self,
        task_id: &str,
        new_state: TaskState,
        owner: &OwnerId,
    ) -> OwnedTaskResult<()> {
        self.validate_ownership(task_id, owner).await?;

        self.inner
            .transition(task_id, new_state)
            .map_err(OwnedTaskError::Database)
    }

    /// Update task fields with ownership validation.
    pub async fn update(
        &self,
        task_id: &str,
        updates: TaskUpdate,
        owner: &OwnerId,
    ) -> OwnedTaskResult<()> {
        self.validate_ownership(task_id, owner).await?;

        self.inner
            .update(task_id, updates)
            .map_err(OwnedTaskError::Database)
    }

    /// Update task with an owner token (for more precise validation).
    pub async fn update_with_token(
        &self,
        task_id: &str,
        updates: TaskUpdate,
        token: &OwnerToken,
    ) -> OwnedTaskResult<()> {
        self.validate_token(task_id, token).await?;

        self.inner
            .update(task_id, updates)
            .map_err(OwnedTaskError::Database)
    }

    /// Validate that the given owner is allowed to mutate the task.
    async fn validate_ownership(&self, task_id: &str, owner: &OwnerId) -> OwnedTaskResult<()> {
        // Check if owner can update
        if !self.ownership.can_update(task_id, owner).await {
            // Get current state for error message
            if let Some(state) = self.ownership.get(task_id).await {
                return Err(OwnedTaskError::StaleOwner {
                    stale: owner.id.parse().unwrap_or(0),
                    current: state.generation,
                });
            }
            return Err(OwnedTaskError::NotOwner);
        }
        Ok(())
    }

    /// Validate that the given token is still valid.
    async fn validate_token(&self, task_id: &str, token: &OwnerToken) -> OwnedTaskResult<()> {
        match self.ownership.validate_token(task_id, token).await {
            Ok(_) => Ok(()),
            Err(OwnershipError::StaleOwner { stale, current }) => {
                Err(OwnedTaskError::StaleOwner { stale, current })
            }
            Err(e) => Err(OwnedTaskError::Ownership(e)),
        }
    }

    /// Check if a task can be updated by the given owner.
    pub async fn can_update(&self, task_id: &str, owner: &OwnerId) -> bool {
        self.ownership.can_update(task_id, owner).await
    }

    /// Check if a task can be updated with the given token.
    pub async fn can_update_with_token(&self, task_id: &str, token: &OwnerToken) -> bool {
        if let Some(state) = self.ownership.get(task_id).await {
            !token.is_stale(state.generation)
        } else {
            true
        }
    }

    /// Get current ownership state for a task.
    pub async fn ownership_state(
        &self,
        task_id: &str,
    ) -> Option<crate::pipeline::ownership::OwnershipState> {
        self.ownership.get(task_id).await
    }

    /// Acquire ownership of a task.
    pub async fn acquire(
        &self,
        task_id: &str,
        owner: OwnerId,
        force: bool,
    ) -> crate::pipeline::ownership::OwnershipResult {
        self.ownership.acquire(task_id, owner, None, force).await
    }

    /// Release ownership of a task.
    pub async fn release(
        &self,
        task_id: &str,
        owner: &OwnerId,
    ) -> crate::pipeline::ownership::OwnershipResult {
        self.ownership.release(task_id, owner).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::ownership::OwnershipManager;
    use crate::store::task::NewTask;

    fn create_test_store() -> (TaskStore<'static>, OwnershipManager) {
        let db = Box::leak(Box::new(
            crate::store::Database::in_memory().expect("Failed to create test DB"),
        ));
        let store = TaskStore::new(db);
        let ownership = OwnershipManager::new();
        (store, ownership)
    }

    #[tokio::test]
    async fn test_owned_task_allows_current_owner() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("owned-test-1".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Acquire ownership
        let result = owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;
        assert!(result.success);

        // Current owner should be able to update
        let can_update = owned_store.can_update(&task.id, &OwnerId::worker(1)).await;
        assert!(can_update);

        // Update should succeed
        let update = TaskUpdate {
            title: Some("Updated Title".to_string()),
            description: None,
            state: None,
            pipeline_phase: None,
            priority: None,
            worktree_path: None,
            worktree_branch: None,
            checkpoint_data: None,
            error: None,
            metadata: None,
            agent_role: None,
            log_file: None,
        };
        let result = owned_store
            .update(&task.id, update, &OwnerId::worker(1))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_owned_task_blocks_stale_owner() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("stale-test-1".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Worker 1 acquires ownership
        owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;

        // Worker 2 force-acquires (Worker 1 is now stale)
        owned_store
            .acquire(&task.id, OwnerId::worker(2), true)
            .await;

        // Worker 1 should NOT be able to update
        let can_update = owned_store.can_update(&task.id, &OwnerId::worker(1)).await;
        assert!(!can_update, "Stale owner should not be able to update");

        // Update should fail
        let update = TaskUpdate {
            title: Some("Stale Update".to_string()),
            description: None,
            state: None,
            pipeline_phase: None,
            priority: None,
            worktree_path: None,
            worktree_branch: None,
            checkpoint_data: None,
            error: None,
            metadata: None,
            agent_role: None,
            log_file: None,
        };
        let result = owned_store
            .update(&task.id, update, &OwnerId::worker(1))
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OwnedTaskError::StaleOwner { .. }
        ));
    }

    #[tokio::test]
    async fn test_owned_task_allows_new_owner() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("new-owner-test".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Worker 1 acquires
        owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;

        // Worker 2 force-acquires
        owned_store
            .acquire(&task.id, OwnerId::worker(2), true)
            .await;

        // Worker 2 should be able to update
        let can_update = owned_store.can_update(&task.id, &OwnerId::worker(2)).await;
        assert!(can_update, "New owner should be able to update");

        // Update should succeed
        let update = TaskUpdate {
            title: Some("New Owner Update".to_string()),
            description: None,
            state: None,
            pipeline_phase: None,
            priority: None,
            worktree_path: None,
            worktree_branch: None,
            checkpoint_data: None,
            error: None,
            metadata: None,
            agent_role: None,
            log_file: None,
        };
        let result = owned_store
            .update(&task.id, update, &OwnerId::worker(2))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_force_transfer_increments_generation() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("gen-transfer-test".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Worker 1 acquires - generation 1
        let r1 = owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;
        assert!(r1.success);
        assert_eq!(r1.state.generation, 1);

        // Worker 2 force-acquires - generation 2
        let r2 = owned_store
            .acquire(&task.id, OwnerId::worker(2), true)
            .await;
        assert!(r2.success);
        assert_eq!(r2.state.generation, 2);

        // Worker 1's token should be stale
        if let Some(token) = &r1.token {
            let can_update = owned_store.can_update_with_token(&task.id, token).await;
            assert!(!can_update, "Original token should be stale after transfer");
        }
    }

    #[tokio::test]
    async fn test_ownership_state_after_transfer() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("state-after-transfer".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Worker 1 acquires
        owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;

        // Check ownership state
        let state = owned_store.ownership_state(&task.id).await;
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.generation, 1);
        assert!(state.is_owner(&OwnerId::worker(1)));

        // Worker 2 force-acquires
        owned_store
            .acquire(&task.id, OwnerId::worker(2), true)
            .await;

        // Check updated ownership state
        let state = owned_store.ownership_state(&task.id).await;
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.generation, 2);
        assert!(state.is_owner(&OwnerId::worker(2)));
    }

    #[tokio::test]
    async fn test_release_allows_new_acquisition() {
        let (store, ownership) = create_test_store();
        let owned_store = OwnedTaskStore::new(&store, &ownership);

        // Create a task
        let task = store
            .create(NewTask {
                id: Some("release-test".to_string()),
                title: "Test Task".to_string(),
                description: None,
                state: None,
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: None,
                project_path: None,
                agent_role: None,
                execution_mode: None,
                repo_root: None,
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })
            .expect("Failed to create task");

        // Worker 1 acquires
        owned_store
            .acquire(&task.id, OwnerId::worker(1), false)
            .await;

        // Worker 1 releases
        let release_result = owned_store.release(&task.id, &OwnerId::worker(1)).await;
        assert!(release_result.success);

        // Now Worker 2 can acquire (without force)
        let result = owned_store
            .acquire(&task.id, OwnerId::worker(2), false)
            .await;
        assert!(result.success);
        assert_eq!(result.state.generation, 2);
    }
}
