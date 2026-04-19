//! Parallel execution engine for decomposed tasks
//!
//! # Status: placeholder scaffold
//!
//! As of writing, `ParallelExecutor` and its owning `DecompositionManager`
//! have **zero callers** in the production code path — the pure-logic
//! decomposition primitives (task decomposer, aggregator, dependency
//! graph) are used by tests only, and the orchestrator does not wire
//! this executor in.
//!
//! [`ParallelExecutor::execute_plan`] intentionally returns
//! [`ParallelExecutionError::NotImplemented`] rather than the previous
//! `sleep(100ms) + return Completed` stub, which silently reported
//! success for work that never ran.

use std::sync::Arc;

use super::types::{ChildTaskStatus, DecompositionId, DecompositionPlan};
use crate::pipeline::queue::TaskQueue;
use crate::pipeline::worker_pool::WorkerPool;

/// Parallel execution engine for decomposed tasks.
///
/// Scaffold only — see module-level docs. The constructor and public
/// method surface are retained so [`DecompositionManager`](super::DecompositionManager)
/// assembly compiles; the fields are held for a future real
/// implementation but are not currently read.
pub struct ParallelExecutor {
    _worker_pool: Arc<WorkerPool>,
    _queue: Arc<TaskQueue>,
    _max_parallelism: usize,
}

impl ParallelExecutor {
    /// Create a new parallel executor.
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        queue: Arc<TaskQueue>,
        max_parallelism: usize,
    ) -> Self {
        Self {
            _worker_pool: worker_pool,
            _queue: queue,
            _max_parallelism: max_parallelism,
        }
    }

    /// Execute a decomposition plan.
    ///
    /// **Not wired in.** Always returns
    /// [`ParallelExecutionError::NotImplemented`]. The previous
    /// sleep-and-claim-success stub was removed — lying about execution
    /// corrupts downstream state in any caller that treats `Ok` as
    /// "work happened." A future real implementation should construct
    /// a [`DependencyGraph`] from the plan, validate it, and dispatch
    /// children level-by-level through the orchestrator's worker pool.
    pub async fn execute_plan(
        &self,
        _plan: &DecompositionPlan,
    ) -> Result<Vec<ChildTaskStatus>, ParallelExecutionError> {
        Err(ParallelExecutionError::NotImplemented(
            "decomposition execution is a scaffold — not wired into the orchestrator"
                .to_string(),
        ))
    }

    /// No-op: nothing to cancel because nothing is ever dispatched.
    /// Retained so `DecompositionManager::cancel_plan` still compiles.
    pub async fn cancel_plan(&self, _plan_id: DecompositionId) {}
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

    /// The decomposition executor scaffold was invoked but is not wired
    /// into a real worker path. Replaces the previous silent-success
    /// stub so callers (and tests) see an explicit failure.
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}
