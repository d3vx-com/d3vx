//! VEX Task Management Module
//!
//! Handles autonomous execution tasks, workspace provisioning, and monitoring.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::phases::{Phase, TaskStatus};
use super::queue::TaskQueue;
use super::queue_manager::QueueManager;
use super::scheduler::TaskScheduler;
use super::task_factory::TaskFactory;

/// Handle for a Vex autonomous execution task
/// Use this to monitor and control a Vex task
#[derive(Debug, Clone)]
pub struct VexTaskHandle {
    /// The task ID in the queue
    pub task_id: String,
    /// The workspace ID (for UI visibility)
    pub workspace_id: String,
    /// Path to the worktree
    pub worktree_path: PathBuf,
}

/// Status of a Vex task
#[derive(Debug, Clone)]
pub struct VexTaskStatus {
    /// Task ID
    pub task_id: String,
    /// Current status
    pub status: TaskStatus,
    /// Current phase
    pub phase: Option<Phase>,
    /// Human-readable message
    pub message: String,
}

pub struct VexManager {
    active_tasks: Arc<RwLock<HashMap<String, String>>>,
    task_factory: Arc<TaskFactory>,
    queue_manager: Arc<QueueManager>,
    scheduler: Arc<TaskScheduler>,
    queue: Arc<TaskQueue>,
}

impl VexManager {
    pub fn new(
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
        task_factory: Arc<TaskFactory>,
        queue_manager: Arc<QueueManager>,
        scheduler: Arc<TaskScheduler>,
        queue: Arc<TaskQueue>,
    ) -> Self {
        Self {
            active_tasks,
            task_factory,
            queue_manager,
            scheduler,
            queue,
        }
    }

    /// Create a Vex autonomous execution task with workspace
    pub async fn create_task(
        &self,
        description: &str,
        project_path: &str,
        branch: Option<&str>,
    ) -> Result<VexTaskHandle> {
        self.task_factory
            .create_vex_task(description, project_path, branch)
            .await
    }

    /// Get a Vex task's current status
    pub async fn get_status(&self, handle: &VexTaskHandle) -> Result<VexTaskStatus> {
        let task = match self.queue.get_task(&handle.task_id).await {
            Some(t) => t,
            None => {
                return Ok(VexTaskStatus {
                    task_id: handle.task_id.clone(),
                    status: TaskStatus::Unknown,
                    phase: None,
                    message: "Task not found in queue".to_string(),
                });
            }
        };

        let status = task.status;
        let phase = Some(task.phase);

        Ok(VexTaskStatus {
            task_id: handle.task_id.clone(),
            status,
            phase,
            message: format!("{:?} - {}", status, task.phase),
        })
    }

    /// Cancel a Vex task
    pub async fn cancel_task(&self, handle: VexTaskHandle) -> Result<()> {
        // Remove from active tasks
        self.active_tasks.write().await.remove(&handle.workspace_id);

        // Cancel in queue
        self.queue_manager.cancel_task(&handle.task_id).await?;

        info!("Cancelled Vex task {}", handle.task_id);
        Ok(())
    }

    /// Dispatch a single Vex task for execution
    pub async fn dispatch_task(&self, handle: &VexTaskHandle) -> Result<()> {
        // Transition to queued status
        self.queue_manager
            .transition_task(&handle.task_id, TaskStatus::Queued)
            .await?;

        // Dispatch - we request 1 parallel slot
        self.scheduler.dispatch_parallel(1, None, None).await?;

        Ok(())
    }
}
