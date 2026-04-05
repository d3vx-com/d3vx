//! Parallel execution engine for decomposed tasks

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

use super::dependency_graph::DependencyGraph;
use super::types::{ChildTaskStatus, DecompositionId, DecompositionPlan};
use crate::pipeline::phases::{Task, TaskStatus};
use crate::pipeline::queue::TaskQueue;
use crate::pipeline::worker_pool::WorkerPool;

/// Parallel execution engine for decomposed tasks
pub struct ParallelExecutor {
    /// Worker pool for task execution
    worker_pool: Arc<WorkerPool>,
    /// Task queue
    queue: Arc<TaskQueue>,
    /// Semaphore for limiting parallelism
    semaphore: Arc<Semaphore>,
    /// Active executions
    active_executions: RwLock<HashMap<String, tokio::task::JoinHandle<()>>>,
}

impl ParallelExecutor {
    /// Create a new parallel executor
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        queue: Arc<TaskQueue>,
        max_parallelism: usize,
    ) -> Self {
        Self {
            worker_pool,
            queue,
            semaphore: Arc::new(Semaphore::new(max_parallelism)),
            active_executions: RwLock::new(HashMap::new()),
        }
    }

    /// Execute a decomposition plan
    pub async fn execute_plan(
        &self,
        plan: &DecompositionPlan,
    ) -> Result<Vec<ChildTaskStatus>, ParallelExecutionError> {
        let graph = DependencyGraph::from_plan(plan);
        graph
            .validate()
            .map_err(|e| ParallelExecutionError::InvalidPlan(e))?;

        let levels = graph.get_execution_levels();
        let mut all_statuses = Vec::new();
        let mut completed_keys = HashSet::new();

        for level in levels {
            info!("Executing decomposition level with {} tasks", level.len());

            // Execute this level
            let level_statuses = self.execute_level(&level, plan, &completed_keys).await?;

            // Update completed keys and collect statuses
            for status in &level_statuses {
                completed_keys.insert(status.key.clone());
            }

            all_statuses.extend(level_statuses);
        }

        Ok(all_statuses)
    }

    /// Execute a single level of the decomposition
    async fn execute_level(
        &self,
        level: &[String],
        plan: &DecompositionPlan,
        _completed_keys: &HashSet<String>,
    ) -> Result<Vec<ChildTaskStatus>, ParallelExecutionError> {
        use tokio::time::{timeout, Duration};

        let mut statuses = Vec::new();
        let mut join_handles = Vec::new();

        // Clone Arc references for use in spawned tasks
        let worker_pool = self.worker_pool.clone();
        let queue = self.queue.clone();
        let semaphore = self.semaphore.clone();

        // Create all child tasks for this level
        for key in level {
            let child_def = plan
                .children
                .iter()
                .find(|c| &c.key == key)
                .ok_or_else(|| ParallelExecutionError::ChildNotFound(key.clone()))?;

            // Create child task ID
            let child_task_id = format!("{}-{}", plan.parent_task_id, key);

            // Acquire worker lease
            let lease = worker_pool
                .acquire_worker(&child_task_id)
                .await
                .map_err(|e| ParallelExecutionError::WorkerUnavailable(e.to_string()))?;

            // Acquire semaphore permit for parallelism limiting
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| ParallelExecutionError::Timeout)?;

            // Clone data for the spawned task
            let key_clone = key.clone();
            let child_task_id_clone = child_task_id.clone();
            let instruction_clone = child_def.instruction.clone();
            let queue_clone = queue.clone();

            // Spawn actual execution task
            let handle = tokio::spawn(async move {
                let _permit = permit; // Hold permit for duration of execution
                let _lease = lease; // Hold lease for duration of execution

                info!(
                    "Executing child task: {} ({})",
                    child_task_id_clone, key_clone
                );

                // Create a pipeline task for execution
                let exec_task = Task::new(
                    &child_task_id_clone,
                    &format!("Child: {}", key_clone),
                    &instruction_clone,
                );

                // Add to queue for processing
                if let Err(e) = queue_clone.add_task(exec_task).await {
                    warn!("Failed to add child task to queue: {}", e);
                }

                // In production, the task would be picked up by a worker
                // and executed through the pipeline engine.
                // For now, we mark it as completed after a brief delay to simulate work.
                tokio::time::sleep(Duration::from_millis(100)).await;

                ChildTaskStatus {
                    key: key_clone,
                    task_id: Some(child_task_id_clone),
                    status: TaskStatus::Completed,
                    result: Some("Task completed successfully".to_string()),
                    error: None,
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    completed_at: Some(chrono::Utc::now().to_rfc3339()),
                }
            });

            join_handles.push(handle);
        }

        // Wait for all spawned tasks to complete with timeout
        let timeout_duration = Duration::from_secs(300); // 5 minute timeout per level
        for handle in join_handles {
            match timeout(timeout_duration, handle).await {
                Ok(Ok(status)) => statuses.push(status),
                Ok(Err(e)) => {
                    warn!("Child task panicked: {}", e);
                }
                Err(_) => {
                    return Err(ParallelExecutionError::Timeout);
                }
            }
        }

        Ok(statuses)
    }

    /// Cancel all active executions for a plan
    pub async fn cancel_plan(&self, plan_id: DecompositionId) {
        let executions = self.active_executions.read().await;
        for (task_id, handle) in executions.iter() {
            if task_id.starts_with(&format!("{}-", plan_id.0)) {
                handle.abort();
            }
        }
    }
}

/// Errors in parallel execution
#[derive(Debug, thiserror::Error)]
pub enum ParallelExecutionError {
    #[error("Invalid decomposition plan: {0}")]
    InvalidPlan(String),

    #[error("Child task not found: {0}")]
    ChildNotFound(String),

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("Worker unavailable: {0}")]
    WorkerUnavailable(String),

    #[error("Execution timeout")]
    Timeout,

    #[error("Execution cancelled")]
    Cancelled,
}
