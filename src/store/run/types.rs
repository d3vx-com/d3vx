//! Task run types and data structures
//!
//! Defines the run status enum and data structs for task execution attempts.

use serde::{Deserialize, Serialize};

/// Status of a task run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunStatus {
    /// Run is waiting to start
    Pending,
    /// Run is currently executing
    Running,
    /// Run completed successfully
    Completed,
    /// Run failed
    Failed,
    /// Run was cancelled
    Cancelled,
}

impl RunStatus {
    /// Get all valid statuses
    pub fn all() -> &'static [RunStatus] {
        &[
            RunStatus::Pending,
            RunStatus::Running,
            RunStatus::Completed,
            RunStatus::Failed,
            RunStatus::Cancelled,
        ]
    }
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Pending => write!(f, "PENDING"),
            RunStatus::Running => write!(f, "RUNNING"),
            RunStatus::Completed => write!(f, "COMPLETED"),
            RunStatus::Failed => write!(f, "FAILED"),
            RunStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(RunStatus::Pending),
            "RUNNING" => Ok(RunStatus::Running),
            "COMPLETED" => Ok(RunStatus::Completed),
            "FAILED" => Ok(RunStatus::Failed),
            "CANCELLED" => Ok(RunStatus::Cancelled),
            _ => Err(format!("Invalid run status: {}", s)),
        }
    }
}

/// A single execution run of a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    /// Unique run identifier
    pub id: String,
    /// Associated task ID
    pub task_id: String,
    /// Attempt number (1-indexed)
    pub attempt_number: i32,
    /// Current status
    pub status: RunStatus,
    /// Worker assigned to this run
    pub worker_id: Option<String>,
    /// Workspace being used for this run
    pub workspace_id: Option<String>,
    /// When the run started
    pub started_at: Option<String>,
    /// When the run ended
    pub ended_at: Option<String>,
    /// Reason for failure (if failed)
    pub failure_reason: Option<String>,
    /// Summary of the run outcome
    pub summary: Option<String>,
    /// Execution metrics (JSON)
    pub metrics_json: String,
}

/// Input for creating a new task run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTaskRun {
    /// Optional custom ID
    pub id: Option<String>,
    /// Associated task ID
    pub task_id: String,
    /// Attempt number
    pub attempt_number: i32,
    /// Worker ID
    pub worker_id: Option<String>,
    /// Workspace ID
    pub workspace_id: Option<String>,
    /// Metrics
    pub metrics: Option<serde_json::Value>,
}

/// Fields to update on a task run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRunUpdate {
    /// New status
    pub status: Option<RunStatus>,
    /// Worker assignment
    pub worker_id: Option<String>,
    /// Workspace assignment
    pub workspace_id: Option<String>,
    /// End timestamp
    pub ended_at: Option<String>,
    /// Failure reason
    pub failure_reason: Option<String>,
    /// Summary
    pub summary: Option<String>,
    /// Updated metrics
    pub metrics: Option<serde_json::Value>,
}

/// Options for listing task runs
#[derive(Debug, Clone, Default)]
pub struct TaskRunListOptions {
    /// Filter by task ID
    pub task_id: Option<String>,
    /// Filter by status
    pub status: Option<Vec<RunStatus>>,
    /// Filter by worker ID
    pub worker_id: Option<String>,
    /// Maximum results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}
