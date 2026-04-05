//! Task Scheduler Module
//!
//! Handles parallel task dispatching, execution monitoring, and resource cleanup.

use anyhow::Result;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use super::checkpoint::CheckpointManager;
use super::engine::{PipelineEngine, PipelineRunResult};
use super::github;
use super::phases::{PhaseContext, Task, TaskStatus};
use super::queue::TaskQueue;
use super::worker_pool::{WorkerLease, WorkerPool};
use crate::agent::AgentLoop;

/// Guard for guaranteed cleanup of task execution resources
pub struct ExecutionGuard {
    worker_pool: Arc<WorkerPool>,
    task_id: String,
    lease: Option<WorkerLease>,
}

impl ExecutionGuard {
    pub fn new(worker_pool: Arc<WorkerPool>, task_id: String, lease: WorkerLease) -> Self {
        Self {
            worker_pool,
            task_id,
            lease: Some(lease),
        }
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() {
            let worker_pool = self.worker_pool.clone();
            let task_id = self.task_id.clone();

            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::try_current();
                if let Ok(handle) = rt {
                    handle.block_on(async {
                        if let Err(e) = worker_pool.release_worker(lease).await {
                            warn!("Failed to release worker for task {}: {}", task_id, e);
                        }
                    });
                }
            });
        }
    }
}

pub struct TaskScheduler {
    worker_pool: Arc<WorkerPool>,
    queue: Arc<TaskQueue>,
    engine: Arc<PipelineEngine>,
    checkpoint_manager: Arc<CheckpointManager>,
    active_tasks: Arc<RwLock<HashMap<String, String>>>,
}

impl TaskScheduler {
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        queue: Arc<TaskQueue>,
        engine: Arc<PipelineEngine>,
        checkpoint_manager: Arc<CheckpointManager>,
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
    ) -> Self {
        Self {
            worker_pool,
            queue,
            engine,
            checkpoint_manager,
            active_tasks,
        }
    }

    pub async fn dispatch_parallel(
        &self,
        max_parallel: usize,
        github_config: Option<crate::config::GitHubIntegration>,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<Vec<PipelineRunResult>> {
        info!("Dispatching tasks in PARALLEL with max: {}", max_parallel);

        let runnable_tasks = self.queue.list_by_status(TaskStatus::Queued).await;
        if runnable_tasks.is_empty() {
            return Ok(Vec::new());
        }

        let limit = max_parallel.min(runnable_tasks.len());
        let tasks_to_run: Vec<_> = runnable_tasks.into_iter().take(limit).collect();

        let mut join_set = JoinSet::new();

        for task in tasks_to_run {
            let task_id = task.id.clone();
            let panic_task = task.clone();

            let worker_pool = self.worker_pool.clone();
            let queue = self.queue.clone();
            let active_tasks = self.active_tasks.clone();
            let engine = self.engine.clone();
            let checkpoint_manager = self.checkpoint_manager.clone();
            let gh_config = github_config.clone();
            let orchestrator_agent = agent.clone();

            join_set.spawn(async move {
                let task_id_for_panic = task_id.clone();
                let panic_future = {
                    let worker_pool = worker_pool.clone();
                    let queue = queue.clone();
                    let active_tasks = active_tasks.clone();
                    let engine = engine.clone();
                    let checkpoint_manager = checkpoint_manager.clone();
                    let gh_config = gh_config.clone();
                    let orchestrator_agent = orchestrator_agent.clone();
                    let task = task.clone();

                    async move {
                        info!("Parallel dispatch: starting task {}", task_id);

                        let lease = match worker_pool.acquire_worker(&task_id).await {
                            Ok(lease) => lease,
                            Err(e) => {
                                error!("Failed to acquire worker for task {}: {}", task_id, e);
                                return PipelineRunResult {
                                    success: false,
                                    task,
                                    phase_results: HashMap::new(),
                                    error: Some(format!("Worker acquisition failed: {}", e)),
                                };
                            }
                        };

                        let worktree = format!(".d3vx/worktrees/{}", task_id);
                        active_tasks
                            .write()
                            .await
                            .insert(task_id.clone(), worktree.clone());

                        if let Err(e) = queue.update_status(&task_id, TaskStatus::InProgress).await
                        {
                            error!("Failed to update task status: {}", e);
                            let _ = worker_pool.release_worker(lease).await;
                            active_tasks.write().await.remove(&task_id);
                            return PipelineRunResult {
                                success: false,
                                task,
                                phase_results: HashMap::new(),
                                error: Some(format!("Status update failed: {}", e)),
                            };
                        }

                        match github::sync_github_task_started(gh_config.clone(), &task).await {
                            Ok(Some(patch)) => {
                                if let Ok(updated_task) =
                                    queue.update_metadata(&task_id, patch).await
                                {
                                    if let Some(mut checkpoint) = checkpoint_manager
                                        .load_checkpoint(&task_id)
                                        .await
                                        .ok()
                                        .flatten()
                                    {
                                        checkpoint.task = updated_task;
                                        let _ =
                                            checkpoint_manager.update_checkpoint(&checkpoint).await;
                                    }
                                }
                            }
                            _ => {}
                        }

                        let _guard =
                            ExecutionGuard::new(worker_pool.clone(), task_id.clone(), lease);

                        let current_dir = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."));
                        let context = PhaseContext::new(
                            task.clone(),
                            current_dir.to_string_lossy().to_string(),
                            worktree,
                        );

                        let mut result = match engine.run(task.clone(), context).await {
                            Ok(result) => result,
                            Err(e) => {
                                error!("Task {} execution failed: {}", task_id, e);
                                let _ = queue.update_status(&task_id, TaskStatus::Failed).await;
                                PipelineRunResult {
                                    success: false,
                                    task,
                                    phase_results: HashMap::new(),
                                    error: Some(e.to_string()),
                                }
                            }
                        };

                        if let Some(updated_task) = queue.get_task(&task_id).await {
                            result.task = updated_task;
                        }

                        let final_status = if result.success {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        };
                        let _ = queue.update_status(&task_id, final_status).await;

                        match github::sync_github_task_finished(
                            gh_config.clone(),
                            &result,
                            orchestrator_agent.clone(),
                        )
                        .await
                        {
                            Ok(Some(patch)) => {
                                if let Ok(updated_task) =
                                    queue.update_metadata(&task_id, patch).await
                                {
                                    result.task = updated_task.clone();
                                    if let Some(mut checkpoint) = checkpoint_manager
                                        .load_checkpoint(&task_id)
                                        .await
                                        .ok()
                                        .flatten()
                                    {
                                        checkpoint.task = updated_task;
                                        let _ =
                                            checkpoint_manager.update_checkpoint(&checkpoint).await;
                                    }
                                }
                            }
                            _ => {}
                        }

                        active_tasks.write().await.remove(&task_id);
                        result
                    }
                };

                match std::panic::AssertUnwindSafe(panic_future)
                    .catch_unwind()
                    .await
                {
                    Ok(result) => result,
                    Err(payload) => {
                        let panic_message = if let Some(message) = payload.downcast_ref::<&str>() {
                            (*message).to_string()
                        } else if let Some(message) = payload.downcast_ref::<String>() {
                            message.clone()
                        } else {
                            "unknown panic payload".to_string()
                        };

                        error!("Task {} panicked: {}", task_id_for_panic, panic_message);
                        let _ = queue
                            .update_status(&task_id_for_panic, TaskStatus::Failed)
                            .await;
                        active_tasks.write().await.remove(&task_id_for_panic);

                        PipelineRunResult {
                            success: false,
                            task: panic_task,
                            phase_results: HashMap::new(),
                            error: Some(format!("Task panicked: {}", panic_message)),
                        }
                    }
                }
            });
        }

        let mut final_results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(run_result) => final_results.push(run_result),
                Err(e) => {
                    error!("Task join failed: {}", e);
                    final_results.push(PipelineRunResult {
                        success: false,
                        task: Task::new("ERROR", "Join Error", "Join failure"),
                        phase_results: HashMap::new(),
                        error: Some(format!("Task join failed: {}", e)),
                    });
                }
            }
        }

        Ok(final_results)
    }
}
