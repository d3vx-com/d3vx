//! Task struct definitions
//!
//! Core data structures for tasks: Task, NewTask, TaskUpdate,
//! TaskListOptions, TaskLog, and database row mapping methods.

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use super::enums::{AgentRole, ExecutionMode};
use super::state_machine::TaskState;
use crate::store::workspace::ScopeMode;

/// A task in the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: String,
    /// Task title
    pub title: String,
    /// Detailed description
    pub description: Option<String>,
    /// Current state
    pub state: TaskState,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Batch ID for grouping related tasks
    pub batch_id: Option<String>,
    /// Git worktree path
    pub worktree_path: Option<String>,
    /// Git worktree branch
    pub worktree_branch: Option<String>,
    /// Current pipeline phase
    pub pipeline_phase: Option<String>,
    /// Checkpoint data for resumption
    pub checkpoint_data: Option<String>,
    /// Number of retries attempted
    pub retry_count: i32,
    /// Maximum allowed retries
    pub max_retries: i32,
    /// Task dependencies (JSON array of task IDs)
    pub depends_on: String,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Additional metadata (JSON object)
    pub metadata: String,
    /// Project path
    pub project_path: Option<String>,
    /// Assigned agent role
    pub agent_role: Option<AgentRole>,
    /// Log file path
    pub log_file: Option<String>,
    /// Execution mode (direct, vex, auto)
    pub execution_mode: ExecutionMode,
    /// Repository root path
    pub repo_root: Option<String>,
    /// Task scope path (subdirectory within repo)
    pub task_scope_path: Option<String>,
    /// Scope mode for execution
    pub scope_mode: ScopeMode,
    /// Parent task ID for hierarchical tasks
    pub parent_task_id: Option<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
    /// When the task started processing
    pub started_at: Option<String>,
    /// When the task completed
    pub completed_at: Option<String>,
}

impl Task {
    /// Map a database row to a Task
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
        let state_str: String = row.get("state")?;
        let role_str: Option<String> = row.get("agent_role")?;
        let execution_mode_str: Option<String> = row.get("execution_mode")?;
        let scope_mode_str: Option<String> = row.get("scope_mode")?;

        Ok(Task {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            state: state_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            priority: row.get("priority")?,
            batch_id: row.get("batch_id")?,
            worktree_path: row.get("worktree_path")?,
            worktree_branch: row.get("worktree_branch")?,
            pipeline_phase: row.get("pipeline_phase")?,
            checkpoint_data: row.get("checkpoint_data")?,
            retry_count: row.get("retry_count")?,
            max_retries: row.get("max_retries")?,
            depends_on: row.get("depends_on")?,
            error: row.get("error")?,
            metadata: row.get("metadata")?,
            project_path: row.get("project_path")?,
            agent_role: role_str
                .map(|s| s.parse().map_err(|_| rusqlite::Error::InvalidQuery))
                .transpose()?,
            log_file: row.get("log_file")?,
            execution_mode: execution_mode_str
                .map(|s| s.parse().map_err(|_| rusqlite::Error::InvalidQuery))
                .transpose()?
                .unwrap_or(ExecutionMode::Auto),
            repo_root: row.get("repo_root")?,
            task_scope_path: row.get("task_scope_path")?,
            scope_mode: scope_mode_str
                .map(|s| s.parse().map_err(|_| rusqlite::Error::InvalidQuery))
                .transpose()?
                .unwrap_or(ScopeMode::Repo),
            parent_task_id: row.get("parent_task_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
        })
    }
}

/// Input for creating a new task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    /// Optional custom ID
    pub id: Option<String>,
    /// Task title
    pub title: String,
    /// Description
    pub description: Option<String>,
    /// Initial state (default: Backlog)
    pub state: Option<TaskState>,
    /// Priority
    pub priority: Option<i32>,
    /// Batch ID
    pub batch_id: Option<String>,
    /// Max retries
    pub max_retries: Option<i32>,
    /// Dependencies
    pub depends_on: Option<Vec<String>>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Project path
    pub project_path: Option<String>,
    /// Agent role
    pub agent_role: Option<AgentRole>,
    /// Execution mode (direct, vex, auto)
    pub execution_mode: Option<ExecutionMode>,
    /// Repository root path
    pub repo_root: Option<String>,
    /// Task scope path (subdirectory within repo)
    pub task_scope_path: Option<String>,
    /// Scope mode for execution
    pub scope_mode: Option<ScopeMode>,
    /// Parent task ID for hierarchical tasks
    pub parent_task_id: Option<String>,
}

impl Default for NewTask {
    fn default() -> Self {
        Self {
            id: None,
            title: String::new(),
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
        }
    }
}

/// Fields to update on a task
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<TaskState>,
    pub priority: Option<i32>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub pipeline_phase: Option<String>,
    pub checkpoint_data: Option<String>,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub agent_role: Option<AgentRole>,
    pub log_file: Option<String>,
}

/// Options for listing tasks
#[derive(Debug, Clone, Default)]
pub struct TaskListOptions {
    pub state: Option<Vec<TaskState>>,
    pub batch_id: Option<String>,
    pub project_path: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// A task log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLog {
    /// Log entry ID
    pub id: i64,
    /// Associated task ID
    pub task_id: String,
    /// Phase name
    pub phase: String,
    /// Event type
    pub event: String,
    /// Event data (JSON)
    pub data: String,
    /// Duration in milliseconds
    pub duration_ms: Option<i64>,
    /// Creation timestamp
    pub created_at: String,
}

impl TaskLog {
    /// Map a database row to a TaskLog
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<TaskLog> {
        Ok(TaskLog {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            phase: row.get("phase")?,
            event: row.get("event")?,
            data: row.get("data")?,
            duration_ms: row.get("duration_ms")?,
            created_at: row.get("created_at")?,
        })
    }
}
