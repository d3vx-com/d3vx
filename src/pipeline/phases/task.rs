//! Task and PhaseContext types
//!
//! Defines the `Task` struct and `PhaseContext` used throughout the pipeline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::{ExecutionMode, Phase, Priority, TaskStatus};

fn default_max_retries() -> u32 {
    3
}

/// A task in the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: String,
    /// Task title/summary
    pub title: String,
    /// Detailed task instruction/description
    pub instruction: String,
    /// Current phase of the task
    pub phase: Phase,
    /// Current status of the task
    pub status: TaskStatus,
    /// Task priority
    #[serde(default)]
    pub priority: Priority,
    /// Worktree path for this task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    /// Git branch for this task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Project root path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    /// Task creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Retry count
    #[serde(default)]
    pub retry_count: u32,
    /// Maximum retries allowed
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Execution mode (Vex, Direct, Auto)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<ExecutionMode>,
}

impl Task {
    /// Create a new task with the given ID and instruction
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        instruction: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            title: title.into(),
            instruction: instruction.into(),
            phase: Phase::default(),
            status: TaskStatus::default(),
            priority: Priority::default(),
            worktree_path: None,
            branch: None,
            project_root: None,
            metadata: serde_json::Value::Null,
            created_at: now,
            updated_at: now,
            retry_count: 0,
            max_retries: 3,
            execution_mode: None,
        }
    }

    /// Builder pattern: set the phase
    pub fn with_phase(mut self, phase: Phase) -> Self {
        self.phase = phase;
        self.updated_at = Utc::now();
        self
    }

    /// Builder pattern: set the status
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self.updated_at = Utc::now();
        self
    }

    /// Builder pattern: set the priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder pattern: set the worktree path
    pub fn with_worktree(mut self, path: impl Into<String>) -> Self {
        self.worktree_path = Some(path.into());
        self.updated_at = Utc::now();
        self
    }

    /// Builder pattern: set the branch
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self.updated_at = Utc::now();
        self
    }

    /// Builder pattern: set the project root
    pub fn with_project_root(mut self, root: impl Into<String>) -> Self {
        self.project_root = Some(root.into());
        self.updated_at = Utc::now();
        self
    }

    /// Builder pattern: set execution mode
    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = Some(mode);
        self.updated_at = Utc::now();
        self
    }

    /// Update the phase and touch the updated_at timestamp
    pub fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.updated_at = Utc::now();
    }

    /// Update the status and touch the updated_at timestamp
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Advance to the next phase
    pub fn advance_phase(&mut self) -> bool {
        if let Some(next) = self.phase.next() {
            self.phase = next;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Check if the task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.updated_at = Utc::now();
    }
}

/// Context passed to phase handlers during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseContext {
    /// Task being executed
    pub task: Task,
    /// Project root directory
    pub project_root: String,
    /// Worktree path for this task
    pub worktree_path: String,
    /// Agent rules from configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_rules: Option<String>,
    /// Memory context for the task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_context: Option<String>,
    /// Ignore instructions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_instruction: Option<String>,
    /// Session ID for the agent (used to create agent if needed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl PhaseContext {
    /// Create a new phase context
    pub fn new(
        task: Task,
        project_root: impl Into<String>,
        worktree_path: impl Into<String>,
    ) -> Self {
        Self {
            task,
            project_root: project_root.into(),
            worktree_path: worktree_path.into(),
            agent_rules: None,
            memory_context: None,
            ignore_instruction: None,
            session_id: None,
        }
    }

    /// Builder pattern: set agent rules
    pub fn with_agent_rules(mut self, rules: impl Into<String>) -> Self {
        self.agent_rules = Some(rules.into());
        self
    }

    /// Builder pattern: set memory context
    pub fn with_memory_context(mut self, context: impl Into<String>) -> Self {
        self.memory_context = Some(context.into());
        self
    }

    /// Builder pattern: set ignore instruction
    pub fn with_ignore_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.ignore_instruction = Some(instruction.into());
        self
    }

    /// Builder pattern: set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}
