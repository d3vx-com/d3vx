//! Queue Manager Module
//!
//! Handles task lifecycle operations: transitions, cancellations, and recovery.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::checkpoint::CheckpointManager;
use super::ownership::{OwnerId, OwnerToken, OwnershipError, OwnershipManager};
use super::phases::{Task, TaskStatus};
use super::queue::{QueueError, TaskQueue};
use super::worker_pool::{WorkerLease, WorkerPool};

#[derive(Debug, thiserror::Error)]
pub enum QueueOwnershipError {
    #[error("Ownership error: {0}")]
    Ownership(#[from] OwnershipError),

    #[error("Queue error: {0}")]
    Queue(#[from] QueueError),

    #[error("Not the current owner of task {0}")]
    NotOwner(String),

    #[error("Stale owner cannot mutate task {task_id}: generation mismatch (stale: {stale}, current: {current})")]
    StaleOwner {
        task_id: String,
        stale: u64,
        current: u64,
    },

    #[error("Task transition failed: {0}")]
    TransitionFailed(String),
}

pub struct QueueManager {
    queue: Arc<TaskQueue>,
    checkpoint_manager: Arc<CheckpointManager>,
    worker_pool: Arc<WorkerPool>,
    active_tasks: Arc<RwLock<HashMap<String, String>>>,
    active_leases: Arc<RwLock<HashMap<String, WorkerLease>>>,
    ownership: Option<Arc<OwnershipManager>>,
}

impl QueueManager {
    pub fn new(
        queue: Arc<TaskQueue>,
        checkpoint_manager: Arc<CheckpointManager>,
        worker_pool: Arc<WorkerPool>,
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
        active_leases: Arc<RwLock<HashMap<String, WorkerLease>>>,
    ) -> Self {
        Self {
            queue,
            checkpoint_manager,
            worker_pool,
            active_tasks,
            active_leases,
            ownership: None,
        }
    }

    pub fn with_ownership(
        queue: Arc<TaskQueue>,
        checkpoint_manager: Arc<CheckpointManager>,
        worker_pool: Arc<WorkerPool>,
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
        active_leases: Arc<RwLock<HashMap<String, WorkerLease>>>,
        ownership: Arc<OwnershipManager>,
    ) -> Self {
        Self {
            queue,
            checkpoint_manager,
            worker_pool,
            active_tasks,
            active_leases,
            ownership: Some(ownership),
        }
    }

    pub fn ownership_manager(&self) -> Option<&Arc<OwnershipManager>> {
        self.ownership.as_ref()
    }

    pub async fn get_task(&self, task_id: &str) -> Option<Task> {
        self.queue.get_task(task_id).await
    }

    pub async fn get_next_task(&self) -> Option<Task> {
        self.queue.get_next().await
    }

    async fn validate_ownership(
        &self,
        task_id: &str,
        owner: &OwnerId,
    ) -> Result<(), QueueOwnershipError> {
        if let Some(ref ownership) = self.ownership {
            if !ownership.can_update(task_id, owner).await {
                if let Some(state) = ownership.get(task_id).await {
                    return Err(QueueOwnershipError::StaleOwner {
                        task_id: task_id.to_string(),
                        stale: owner.id.parse().unwrap_or(0),
                        current: state.generation,
                    });
                }
                return Err(QueueOwnershipError::NotOwner(task_id.to_string()));
            }
        }
        Ok(())
    }

    async fn validate_token(
        &self,
        task_id: &str,
        token: &OwnerToken,
    ) -> Result<(), QueueOwnershipError> {
        if let Some(ref ownership) = self.ownership {
            ownership.validate_token(task_id, token).await?;
        }
        Ok(())
    }

    pub async fn transition_task(&self, task_id: &str, new_status: TaskStatus) -> Result<Task> {
        info!("Transitioning task {} to {}", task_id, new_status);

        let task = self
            .queue
            .update_status(task_id, new_status)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to transition task: {}", e))?;

        // Update checkpoint
        if let Ok(Some(mut checkpoint)) = self.checkpoint_manager.load_checkpoint(task_id).await {
            checkpoint.task = task.clone();
            let _ = self.checkpoint_manager.update_checkpoint(&checkpoint).await;
        }

        Ok(task)
    }

    pub async fn transition_task_owned(
        &self,
        task_id: &str,
        new_status: TaskStatus,
        owner: OwnerId,
    ) -> Result<Task, QueueOwnershipError> {
        self.validate_ownership(task_id, &owner).await?;

        info!(
            "Transitioning task {} to {} (owner: {})",
            task_id, new_status, owner
        );

        let task = self
            .queue
            .update_status(task_id, new_status)
            .await
            .map_err(|e| QueueOwnershipError::TransitionFailed(e.to_string()))?;

        if let Ok(Some(mut checkpoint)) = self.checkpoint_manager.load_checkpoint(task_id).await {
            checkpoint.task = task.clone();
            let _ = self.checkpoint_manager.update_checkpoint(&checkpoint).await;
        }

        Ok(task)
    }

    pub async fn transition_task_with_token(
        &self,
        task_id: &str,
        new_status: TaskStatus,
        token: OwnerToken,
    ) -> Result<Task, QueueOwnershipError> {
        self.validate_token(task_id, &token).await?;

        info!(
            "Transitioning task {} to {} (token owner)",
            task_id, new_status
        );

        let task = self
            .queue
            .update_status(task_id, new_status)
            .await
            .map_err(|e| QueueOwnershipError::TransitionFailed(e.to_string()))?;

        if let Ok(Some(mut checkpoint)) = self.checkpoint_manager.load_checkpoint(task_id).await {
            checkpoint.task = task.clone();
            let _ = self.checkpoint_manager.update_checkpoint(&checkpoint).await;
        }

        Ok(task)
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        info!("Cancelling task: {}", task_id);

        // Release worker lease if held
        if let Some(lease) = self.active_leases.write().await.remove(task_id) {
            let _ = self.worker_pool.release_worker(lease).await;
        }

        // Update status
        let _ = self.queue.update_status(task_id, TaskStatus::Failed).await;

        // Remove from active tasks
        self.active_tasks.write().await.remove(task_id);

        // Delete checkpoint
        let _ = self.checkpoint_manager.delete_checkpoint(task_id).await;

        Ok(())
    }

    pub async fn cancel_task_owned(
        &self,
        task_id: &str,
        owner: OwnerId,
    ) -> Result<(), QueueOwnershipError> {
        self.validate_ownership(task_id, &owner).await?;

        info!("Cancelling task {} (owner: {})", task_id, owner);

        // Release worker lease if held
        if let Some(lease) = self.active_leases.write().await.remove(task_id) {
            let _ = self.worker_pool.release_worker(lease).await;
        }

        // Update status
        let _ = self.queue.update_status(task_id, TaskStatus::Failed).await;

        // Remove from active tasks
        self.active_tasks.write().await.remove(task_id);

        // Delete checkpoint
        let _ = self.checkpoint_manager.delete_checkpoint(task_id).await;

        // Release ownership
        if let Some(ref ownership) = self.ownership {
            let _ = ownership.release(task_id, &owner).await;
        }

        Ok(())
    }

    pub async fn cancel_task_with_token(
        &self,
        task_id: &str,
        token: OwnerToken,
    ) -> Result<(), QueueOwnershipError> {
        self.validate_token(task_id, &token).await?;

        info!("Cancelling task {} (token owner)", task_id);

        // Release worker lease if held
        if let Some(lease) = self.active_leases.write().await.remove(task_id) {
            let _ = self.worker_pool.release_worker(lease).await;
        }

        // Update status
        let _ = self.queue.update_status(task_id, TaskStatus::Failed).await;

        // Remove from active tasks
        self.active_tasks.write().await.remove(task_id);

        // Delete checkpoint
        let _ = self.checkpoint_manager.delete_checkpoint(task_id).await;

        Ok(())
    }

    pub async fn recover_interrupted_tasks(&self, enable_auto_recovery: bool) -> Result<Vec<Task>> {
        if !enable_auto_recovery {
            debug!("Auto-recovery disabled");
            return Ok(Vec::new());
        }

        info!("Checking for resumable checkpoints...");

        let resumable = self.checkpoint_manager.get_resumable_checkpoints().await?;
        let mut recovered = Vec::new();

        for checkpoint in resumable {
            info!("Recovering task: {}", checkpoint.task.id);

            // Transition to queued
            let task = self
                .transition_task(&checkpoint.task.id, TaskStatus::Queued)
                .await?;
            recovered.push(task)
        }

        info!("Recovered {} interrupted tasks", recovered.len());
        Ok(recovered)
    }

    pub async fn patch_task_metadata(
        &self,
        task_id: &str,
        patch: serde_json::Value,
    ) -> Result<Task> {
        let task = self.queue.update_metadata(task_id, patch).await?;
        if let Some(mut checkpoint) = self.checkpoint_manager.load_checkpoint(task_id).await? {
            checkpoint.task = task.clone();
            self.checkpoint_manager
                .update_checkpoint(&checkpoint)
                .await?;
        }
        Ok(task)
    }

    pub async fn patch_task_metadata_owned(
        &self,
        task_id: &str,
        patch: serde_json::Value,
        owner: OwnerId,
    ) -> Result<Task, QueueOwnershipError> {
        self.validate_ownership(task_id, &owner).await?;

        let task = self
            .queue
            .update_metadata(task_id, patch)
            .await
            .map_err(QueueOwnershipError::Queue)?;

        if let Ok(Some(mut checkpoint)) = self.checkpoint_manager.load_checkpoint(task_id).await {
            checkpoint.task = task.clone();
            let _ = self.checkpoint_manager.update_checkpoint(&checkpoint).await;
        }
        Ok(task)
    }

    pub async fn patch_task_metadata_with_token(
        &self,
        task_id: &str,
        patch: serde_json::Value,
        token: OwnerToken,
    ) -> Result<Task, QueueOwnershipError> {
        self.validate_token(task_id, &token).await?;

        let task = self
            .queue
            .update_metadata(task_id, patch)
            .await
            .map_err(QueueOwnershipError::Queue)?;

        if let Ok(Some(mut checkpoint)) = self.checkpoint_manager.load_checkpoint(task_id).await {
            checkpoint.task = task.clone();
            let _ = self.checkpoint_manager.update_checkpoint(&checkpoint).await;
        }
        Ok(task)
    }

    pub async fn can_update(&self, task_id: &str, owner: &OwnerId) -> bool {
        if let Some(ref ownership) = self.ownership {
            ownership.can_update(task_id, owner).await
        } else {
            true
        }
    }

    pub async fn can_update_with_token(&self, task_id: &str, token: &OwnerToken) -> bool {
        if let Some(ref ownership) = self.ownership {
            ownership.validate_token(task_id, token).await.is_ok()
        } else {
            true
        }
    }

    pub async fn get_ownership_state(
        &self,
        task_id: &str,
    ) -> Option<super::ownership::OwnershipState> {
        if let Some(ref ownership) = self.ownership {
            ownership.get(task_id).await
        } else {
            None
        }
    }
}
