//! Recovery and Reconciliation Module
//!
//! Provides startup recovery and reconciliation for tasks, workers, and workspaces.
//! Ensures the system can recover state after restart and handle failures gracefully.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::heartbeat::HeartbeatManager;
use super::worker_pool::WorkerId;
use crate::store::task::TaskState;

/// Recovery configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum age for a task to be considered recoverable
    pub max_task_age: Duration,
    /// Time after which a worker without heartbeat is considered stale
    pub stale_worker_timeout: Duration,
    /// Time after which a workspace is considered abandoned
    pub abandoned_workspace_timeout: Duration,
    /// Whether to automatically requeue recoverable tasks
    pub auto_requeue: bool,
    /// Maximum retries for a task
    pub max_task_retries: u32,
    /// Whether to clean up abandoned workspaces on startup
    pub cleanup_abandoned_workspaces: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_task_age: Duration::from_secs(7 * 24 * 3600), // 7 days
            stale_worker_timeout: Duration::from_secs(300),   // 5 minutes
            abandoned_workspace_timeout: Duration::from_secs(3600), // 1 hour
            auto_requeue: true,
            max_task_retries: 3,
            cleanup_abandoned_workspaces: true,
        }
    }
}

/// Result of recovery operation
#[derive(Debug, Clone, Default)]
pub struct RecoveryResult {
    /// Tasks that were recovered
    pub recovered_tasks: Vec<String>,
    /// Tasks that were requeued
    pub requeued_tasks: Vec<String>,
    /// Tasks that were marked failed
    pub failed_tasks: Vec<String>,
    /// Workers that were marked stale
    pub stale_workers: Vec<WorkerId>,
    /// Workspaces that were cleaned up
    pub cleaned_workspaces: Vec<String>,
    /// Workspaces that were reclaimed
    pub reclaimed_workspaces: Vec<String>,
    /// Errors encountered during recovery
    pub errors: Vec<String>,
}

/// Task recovery info
#[derive(Debug, Clone)]
pub struct TaskRecoveryInfo {
    /// Task ID
    pub task_id: String,
    /// Current state
    pub state: TaskState,
    /// Whether the task had an active run
    pub has_active_run: bool,
    /// Whether the task was assigned to a worker
    pub was_assigned: bool,
    /// Number of previous attempts
    pub attempt_count: u32,
    /// Whether the task is safe to retry
    pub safe_to_retry: bool,
    /// Reason for recovery status
    pub reason: String,
}

/// Workspace recovery info
#[derive(Debug, Clone)]
pub struct WorkspaceRecoveryInfo {
    /// Workspace ID
    pub workspace_id: String,
    /// Associated task ID
    pub task_id: Option<String>,
    /// Workspace path
    pub path: PathBuf,
    /// Whether workspace still exists on disk
    pub exists_on_disk: bool,
}

/// The recovery manager
pub struct RecoveryManager {
    /// Configuration
    config: RecoveryConfig,
    /// Heartbeat manager reference
    heartbeat_manager: Option<Arc<HeartbeatManager>>,
    /// Last recovery time
    last_recovery: Mutex<Option<Instant>>,
}

impl RecoveryManager {
    /// Create a new recovery manager
    pub fn new(config: RecoveryConfig) -> Self {
        Self {
            config,
            heartbeat_manager: None,
            last_recovery: Mutex::new(None),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RecoveryConfig::default())
    }

    /// Set heartbeat manager
    pub fn with_heartbeat_manager(mut self, manager: Arc<HeartbeatManager>) -> Self {
        self.heartbeat_manager = Some(manager);
        self
    }

    /// Run startup recovery using task and workspace information
    pub async fn run_startup_recovery(
        &self,
        incomplete_tasks: Vec<TaskRecoveryInfo>,
        workspace_paths: Vec<(String, PathBuf)>,
    ) -> RecoveryResult {
        info!("Starting recovery process...");
        let start = Instant::now();
        let mut result = RecoveryResult::default();

        // 1. Process incomplete tasks
        info!("Found {} incomplete tasks", incomplete_tasks.len());
        for task_info in incomplete_tasks {
            if self.config.auto_requeue && task_info.safe_to_retry {
                result.requeued_tasks.push(task_info.task_id.clone());
                info!(
                    "Task {} marked for requeue: {}",
                    task_info.task_id, task_info.reason
                );
            } else if !task_info.safe_to_retry {
                result.failed_tasks.push(task_info.task_id.clone());
                warn!(
                    "Task {} cannot be recovered: {}",
                    task_info.task_id, task_info.reason
                );
            }
        }

        // 2. Check for stale workers
        if let Some(ref heartbeat_manager) = self.heartbeat_manager {
            let stale = heartbeat_manager.detect_stale_workers().await;
            for worker_info in stale {
                result.stale_workers.push(worker_info.worker_id);
                warn!("Detected stale worker: {}", worker_info.worker_id);
            }
        }

        // 3. Check workspaces
        for (workspace_id, path) in workspace_paths {
            let exists = path.exists();
            if exists {
                result.reclaimed_workspaces.push(workspace_id.clone());
                debug!("Workspace {} exists at {:?}", workspace_id, path);
            } else {
                result.cleaned_workspaces.push(workspace_id.clone());
                warn!("Workspace {} path missing: {:?}", workspace_id, path);
            }
        }

        // Update last recovery time
        {
            let mut last = self.last_recovery.lock().await;
            *last = Some(start);
        }

        info!(
            "Recovery completed in {:?}: {} recovered, {} requeued, {} failed, {} stale workers, {} cleaned workspaces",
            start.elapsed(),
            result.recovered_tasks.len(),
            result.requeued_tasks.len(),
            result.failed_tasks.len(),
            result.stale_workers.len(),
            result.cleaned_workspaces.len()
        );

        result
    }

    /// Analyze a task for recovery
    pub fn analyze_task_for_recovery(
        &self,
        task_id: &str,
        state: TaskState,
        has_active_run: bool,
        retry_count: u32,
    ) -> TaskRecoveryInfo {
        let safe_to_retry = is_safe_to_retry(&state, retry_count, self.config.max_task_retries);
        let reason = if safe_to_retry {
            "Task can be safely retried".to_string()
        } else if retry_count >= self.config.max_task_retries {
            format!("Exceeded max retries ({})", self.config.max_task_retries)
        } else {
            format!("Task in state {} cannot be retried", state)
        };

        TaskRecoveryInfo {
            task_id: task_id.to_string(),
            state,
            has_active_run,
            was_assigned: has_active_run,
            attempt_count: retry_count,
            safe_to_retry,
            reason,
        }
    }

    /// Run periodic health check
    pub async fn run_health_check(&self) -> RecoveryResult {
        let mut result = RecoveryResult::default();

        // Check for stale workers
        if let Some(ref heartbeat_manager) = self.heartbeat_manager {
            let stale = heartbeat_manager.detect_stale_workers().await;
            for worker_info in stale {
                result.stale_workers.push(worker_info.worker_id);
                warn!("Health check: stale worker {}", worker_info.worker_id);
            }

            // Update heartbeat stats
            let stats = heartbeat_manager.stats().await;
            if stats.stale_workers > 0 {
                result
                    .errors
                    .push(format!("{} stale workers detected", stats.stale_workers));
            }
        }

        result
    }

    /// Get time since last recovery
    pub async fn time_since_last_recovery(&self) -> Option<Duration> {
        let last = self.last_recovery.lock().await;
        last.map(|t| t.elapsed())
    }

    /// Get configuration
    pub fn config(&self) -> &RecoveryConfig {
        &self.config
    }
}

/// Check if a state is terminal
pub fn is_terminal_state(state: &TaskState) -> bool {
    matches!(state, TaskState::Done | TaskState::Failed)
}

/// Check if a task is safe to retry
pub fn is_safe_to_retry(state: &TaskState, retry_count: u32, max_retries: u32) -> bool {
    if retry_count >= max_retries {
        return false;
    }
    // Failed tasks can be retried
    if *state == TaskState::Failed {
        return true;
    }
    // Most non-terminal states are safe to retry
    !is_terminal_state(state)
}

/// Reconciliation actions for a single task
#[derive(Debug, Clone)]
pub enum ReconcileAction {
    /// Task is healthy, no action needed
    NoAction,
    /// Requeue the task
    Requeue,
    /// Mark task as failed
    MarkFailed(String),
    /// Resume task execution
    Resume,
    /// Cancel the task
    Cancel(String),
}

/// Task reconciler for determining what action to take
pub struct TaskReconciler {
    config: RecoveryConfig,
}

impl TaskReconciler {
    /// Create a new reconciler
    pub fn new(config: RecoveryConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn with_defaults() -> Self {
        Self::new(RecoveryConfig::default())
    }

    /// Determine reconciliation action for a task
    pub fn reconcile(
        &self,
        state: &TaskState,
        has_active_run: bool,
        retry_count: u32,
    ) -> ReconcileAction {
        // Terminal states need no action
        if is_terminal_state(state) {
            return ReconcileAction::NoAction;
        }

        // Check retry limit
        if retry_count >= self.config.max_task_retries {
            return ReconcileAction::MarkFailed(format!(
                "Exceeded maximum retries ({})",
                self.config.max_task_retries
            ));
        }

        // Active states without runs should be requeued
        let active_states = [
            TaskState::Research,
            TaskState::Plan,
            TaskState::Implement,
            TaskState::Validate,
            TaskState::Analyze,
            TaskState::AddNew,
            TaskState::Migrate,
            TaskState::RemoveOld,
            TaskState::Reproduce,
            TaskState::Investigate,
            TaskState::Fix,
            TaskState::Harden,
            TaskState::Preparing,
            TaskState::Spawning,
            TaskState::Prepare,
            TaskState::Test,
            TaskState::Execute,
            TaskState::Cleanup,
            TaskState::Review,
            TaskState::Docs,
            TaskState::Learn,
        ];

        if active_states.contains(state) && !has_active_run {
            return ReconcileAction::Requeue;
        }

        // Queued/Ready states should resume
        match state {
            TaskState::Queued | TaskState::Backlog => ReconcileAction::Resume,
            _ => {
                if has_active_run {
                    ReconcileAction::NoAction
                } else {
                    ReconcileAction::Requeue
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_terminal_state() {
        assert!(is_terminal_state(&TaskState::Done));
        assert!(is_terminal_state(&TaskState::Failed));
        assert!(!is_terminal_state(&TaskState::Queued));
        assert!(!is_terminal_state(&TaskState::Implement));
    }

    #[test]
    fn test_is_safe_to_retry() {
        assert!(is_safe_to_retry(&TaskState::Queued, 0, 3));
        assert!(is_safe_to_retry(&TaskState::Implement, 0, 3));
        assert!(is_safe_to_retry(&TaskState::Failed, 0, 3));
        assert!(!is_safe_to_retry(&TaskState::Implement, 3, 3));
        assert!(!is_safe_to_retry(&TaskState::Done, 0, 3));
    }

    #[test]
    fn test_recovery_config_defaults() {
        let config = RecoveryConfig::default();
        assert!(config.auto_requeue);
        assert_eq!(config.max_task_retries, 3);
    }

    #[test]
    fn test_task_reconciler() {
        let reconciler = TaskReconciler::with_defaults();

        // Active state with no run should be requeued
        let action = reconciler.reconcile(&TaskState::Implement, false, 0);
        assert!(matches!(action, ReconcileAction::Requeue));

        // Active state with active run should have no action
        let action = reconciler.reconcile(&TaskState::Implement, true, 0);
        assert!(matches!(action, ReconcileAction::NoAction));

        // Task at retry limit should fail
        let action = reconciler.reconcile(&TaskState::Implement, false, 3);
        assert!(matches!(action, ReconcileAction::MarkFailed(_)));

        // Terminal state should have no action
        let action = reconciler.reconcile(&TaskState::Done, false, 0);
        assert!(matches!(action, ReconcileAction::NoAction));
    }

    #[tokio::test]
    async fn test_recovery_manager() {
        let manager = RecoveryManager::with_defaults();

        let tasks = vec![TaskRecoveryInfo {
            task_id: "TASK-001".to_string(),
            state: TaskState::Implement,
            has_active_run: false,
            was_assigned: false,
            attempt_count: 0,
            safe_to_retry: true,
            reason: "Test".to_string(),
        }];

        let workspaces = vec![];

        let result = manager.run_startup_recovery(tasks, workspaces).await;
        assert_eq!(result.requeued_tasks.len(), 1);
    }
}
