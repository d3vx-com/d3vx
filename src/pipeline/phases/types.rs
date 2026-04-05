//! Pipeline phase type definitions
//!
//! Defines `Phase`, `TaskStatus`, and `Priority` enums.

use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export ExecutionMode from classifier
pub use super::super::classifier::ExecutionMode;

/// Pipeline phases representing the stages of task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    /// Research phase: Analyzes requirements, gathers context
    Research,
    /// Ideation phase: explores alternatives, asks clarifying questions
    Ideation,
    /// Plan phase: Creates high-level implementation plan
    Plan,
    /// Draft phase: Generates implementation drafts (unified diffs)
    Draft,
    /// Implement phase: Executes the implementation (applies diffs)
    Implement,
    /// Review phase: Reviews changes, runs tests
    Review,
    /// Docs phase: Generates documentation
    Docs,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Research
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Phase::Research => write!(f, "RESEARCH"),
            Phase::Ideation => write!(f, "IDEATION"),
            Phase::Plan => write!(f, "PLAN"),
            Phase::Draft => write!(f, "DRAFT"),
            Phase::Implement => write!(f, "IMPLEMENT"),
            Phase::Review => write!(f, "REVIEW"),
            Phase::Docs => write!(f, "DOCS"),
        }
    }
}

impl Phase {
    /// Get the label for this phase
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Research => "Research",
            Phase::Ideation => "Ideation",
            Phase::Plan => "Plan",
            Phase::Draft => "Draft",
            Phase::Implement => "Implement",
            Phase::Review => "Review",
            Phase::Docs => "Docs",
        }
    }

    /// Get the commit prefix for this phase
    pub fn commit_prefix(&self) -> &'static str {
        match self {
            Phase::Research => "chore(research)",
            Phase::Ideation => "chore(ideation)",
            Phase::Plan => "docs(plan)",
            Phase::Draft => "feat(draft)",
            Phase::Implement => "feat",
            Phase::Review => "chore(review)",
            Phase::Docs => "docs",
        }
    }

    /// Get the next phase in the pipeline
    pub fn next(&self) -> Option<Phase> {
        match self {
            Phase::Research => Some(Phase::Ideation),
            Phase::Ideation => Some(Phase::Plan),
            Phase::Plan => Some(Phase::Draft),
            Phase::Draft => Some(Phase::Review),
            Phase::Review => Some(Phase::Implement),
            Phase::Implement => Some(Phase::Docs),
            Phase::Docs => None,
        }
    }

    /// Check if this is the final phase
    pub fn is_final(&self) -> bool {
        matches!(self, Phase::Docs)
    }

    /// Get all phases in order
    pub fn all() -> &'static [Phase] {
        &[
            Phase::Research,
            Phase::Ideation,
            Phase::Plan,
            Phase::Draft,
            Phase::Review,
            Phase::Implement,
            Phase::Docs,
        ]
    }

    /// Parse from string (case-insensitive)
    pub fn from_str_ignore_case(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "RESEARCH" => Some(Phase::Research),
            "IDEATION" => Some(Phase::Ideation),
            "PLAN" => Some(Phase::Plan),
            "DRAFT" => Some(Phase::Draft),
            "IMPLEMENT" => Some(Phase::Implement),
            "REVIEW" => Some(Phase::Review),
            "DOCS" => Some(Phase::Docs),
            _ => None,
        }
    }
}

/// Task status in the pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    /// Task is in backlog, not yet prioritized
    Backlog,
    /// Task is queued for execution
    Queued,
    /// Task is currently in progress
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed during execution
    Failed,
    /// Task was cancelled by user
    Cancelled,
    /// Task status unknown
    Unknown,
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Backlog
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Backlog => write!(f, "BACKLOG"),
            TaskStatus::Queued => write!(f, "QUEUED"),
            TaskStatus::InProgress => write!(f, "IN_PROGRESS"),
            TaskStatus::Completed => write!(f, "COMPLETED"),
            TaskStatus::Failed => write!(f, "FAILED"),
            TaskStatus::Cancelled => write!(f, "CANCELLED"),
            TaskStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl TaskStatus {
    /// Check if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Check if the task is active (being processed)
    pub fn is_active(&self) -> bool {
        matches!(self, TaskStatus::Queued | TaskStatus::InProgress)
    }
}

/// Priority level for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Low priority
    Low = 1,
    /// Normal priority (default)
    Normal = 2,
    /// High priority
    High = 3,
    /// Critical priority (highest)
    Critical = 4,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Normal => write!(f, "normal"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}
