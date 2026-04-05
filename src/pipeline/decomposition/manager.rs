//! Decomposition manager - coordinates the full decomposition lifecycle

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tracing::info;

use super::aggregator::ResultAggregator;
use super::decomposer::TaskDecomposer;
use super::executor::ParallelExecutor;
use super::types::{
    ChildTaskDefinition, ChildTaskStatus, DecompositionId, DecompositionPlan, DecompositionStatus,
    ExecutionStrategy,
};
use crate::pipeline::phases::Task;
use crate::pipeline::queue::TaskQueue;
use crate::pipeline::worker_pool::WorkerPool;

/// Decomposition manager - coordinates the full decomposition lifecycle
pub struct DecompositionManager {
    /// Task decomposer
    decomposer: TaskDecomposer,
    /// Parallel executor
    executor: ParallelExecutor,
    /// Active decomposition plans
    plans: RwLock<HashMap<DecompositionId, DecompositionPlan>>,
    /// Child task statuses by decomposition ID
    child_statuses: Mutex<HashMap<DecompositionId, Vec<ChildTaskStatus>>>,
}

impl DecompositionManager {
    /// Create a new decomposition manager
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        queue: Arc<TaskQueue>,
        max_parallelism: usize,
    ) -> Self {
        Self {
            decomposer: TaskDecomposer::new(),
            executor: ParallelExecutor::new(worker_pool, queue, max_parallelism),
            plans: RwLock::new(HashMap::new()),
            child_statuses: Mutex::new(HashMap::new()),
        }
    }

    /// Create a decomposition plan for a task
    pub async fn create_plan(
        &self,
        parent_task: &Task,
        children: Vec<ChildTaskDefinition>,
        strategy: Option<ExecutionStrategy>,
    ) -> DecompositionId {
        let mut plan = self.decomposer.create_plan(parent_task, children);

        if let Some(s) = strategy {
            plan.execution_strategy = s;
        }

        let id = plan.id;
        self.plans.write().await.insert(id, plan);

        info!(
            "Created decomposition plan {} for task {}",
            id, parent_task.id
        );
        id
    }

    /// Get a decomposition plan by ID
    pub async fn get_plan(&self, id: DecompositionId) -> Option<DecompositionPlan> {
        self.plans.read().await.get(&id).cloned()
    }

    /// Approve a decomposition plan
    pub async fn approve_plan(&self, id: DecompositionId) -> Result<(), DecompositionError> {
        let mut plans = self.plans.write().await;
        let plan = plans
            .get_mut(&id)
            .ok_or(DecompositionError::PlanNotFound(id))?;

        if plan.status != DecompositionStatus::Planned {
            return Err(DecompositionError::InvalidStatus(plan.status));
        }

        plan.approve();
        info!("Approved decomposition plan {}", id);
        Ok(())
    }

    /// Execute a decomposition plan
    pub async fn execute_plan(
        &self,
        id: DecompositionId,
    ) -> Result<DecompositionStatus, DecompositionError> {
        let mut plans = self.plans.write().await;
        let plan = plans
            .get_mut(&id)
            .ok_or(DecompositionError::PlanNotFound(id))?;

        if plan.status != DecompositionStatus::Approved {
            return Err(DecompositionError::InvalidStatus(plan.status));
        }

        plan.start_execution();
        drop(plans);

        // Execute children
        let plan_ref = self.plans.read().await;
        let plan_clone = plan_ref.get(&id).unwrap().clone();
        drop(plan_ref);

        let statuses = self
            .executor
            .execute_plan(&plan_clone)
            .await
            .map_err(|e| DecompositionError::ExecutionError(e.to_string()))?;

        // Aggregate results
        let aggregator = ResultAggregator::new(plan_clone.aggregation_strategy);
        let (status, result) = aggregator.aggregate(&statuses);

        // Update plan
        let mut plans = self.plans.write().await;
        let plan = plans.get_mut(&id).unwrap();

        match status {
            DecompositionStatus::Completed => plan.complete(result),
            DecompositionStatus::Failed => plan.fail(&result),
            DecompositionStatus::Partial => plan.partial(result),
            _ => plan.fail("Unexpected status"),
        }

        // Store child statuses
        self.child_statuses.lock().await.insert(id, statuses);

        info!("Decomposition {} completed with status {:?}", id, status);
        Ok(status)
    }

    /// Cancel a decomposition plan
    pub async fn cancel_plan(&self, id: DecompositionId) -> Result<(), DecompositionError> {
        let mut plans = self.plans.write().await;
        let plan = plans
            .get_mut(&id)
            .ok_or(DecompositionError::PlanNotFound(id))?;

        plan.status = DecompositionStatus::Cancelled;
        plan.completed_at = Some(chrono::Utc::now().to_rfc3339());

        self.executor.cancel_plan(id).await;

        info!("Cancelled decomposition plan {}", id);
        Ok(())
    }

    /// Get child statuses for a decomposition
    pub async fn get_child_statuses(&self, id: DecompositionId) -> Option<Vec<ChildTaskStatus>> {
        self.child_statuses.lock().await.get(&id).cloned()
    }

    /// List all active decompositions
    pub async fn list_active(&self) -> Vec<DecompositionPlan> {
        self.plans
            .read()
            .await
            .values()
            .filter(|p| {
                matches!(
                    p.status,
                    DecompositionStatus::Executing | DecompositionStatus::Approved
                )
            })
            .cloned()
            .collect()
    }
}

/// Errors in decomposition operations
#[derive(Debug, thiserror::Error)]
pub enum DecompositionError {
    #[error("Plan not found: {0}")]
    PlanNotFound(DecompositionId),

    #[error("Invalid status: {0:?}")]
    InvalidStatus(DecompositionStatus),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}
