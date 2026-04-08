//! Task state machine and transitions
//!
//! Defines the TaskState enum and its valid state transitions.

use serde::{Deserialize, Serialize};

/// Task state in the pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    Backlog,
    Queued,
    Research,
    Plan,
    Implement,
    Validate,
    Analyze,
    AddNew,
    Migrate,
    RemoveOld,
    Reproduce,
    Investigate,
    Fix,
    Harden,
    Preparing,
    Spawning,
    Prepare,
    Test,
    Execute,
    Cleanup,
    Review, // Legacy
    Docs,   // Legacy
    Learn,  // Legacy
    Done,
    Failed,
}

impl TaskState {
    /// Get all valid states
    pub fn all() -> &'static [TaskState] {
        &[
            TaskState::Backlog,
            TaskState::Queued,
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
            TaskState::Done,
            TaskState::Failed,
        ]
    }

    /// Get valid transitions from this state
    pub fn valid_transitions(&self) -> Vec<TaskState> {
        use TaskState::*;
        match self {
            Backlog => vec![Queued, Failed],
            Queued => vec![
                Preparing,
                Spawning,
                Research,
                Plan,
                Implement,
                Validate,
                Analyze,
                AddNew,
                Migrate,
                RemoveOld,
                Reproduce,
                Investigate,
                Fix,
                Harden,
                Prepare,
                Test,
                Execute,
                Cleanup,
                Failed,
            ],
            Preparing => vec![Spawning, Research, Plan, Implement, Failed],
            Spawning => vec![Research, Implement, Failed],
            Research => vec![Plan, Failed],
            Plan => vec![Implement, Failed],
            Implement => vec![Validate, Review, Failed],
            Validate => vec![Done, Implement, Failed],
            Analyze => vec![Plan, Failed],
            AddNew => vec![Migrate, Failed],
            Migrate => vec![RemoveOld, Failed],
            RemoveOld => vec![Validate, Failed],
            Reproduce => vec![Investigate, Failed],
            Investigate => vec![Implement, Failed],
            Fix => vec![Harden, Failed],
            Harden => vec![Validate, Done, Failed],
            Prepare => vec![Test, Failed],
            Test => vec![Execute, Failed],
            Execute => vec![Cleanup, Failed],
            Cleanup => vec![Validate, Done, Failed],
            Review => vec![Implement, Docs, Failed],
            Docs => vec![Learn, Done, Failed],
            Learn => vec![Done, Failed],
            Done => vec![],
            Failed => vec![Queued],
        }
    }

    /// Check if a transition to another state is valid
    pub fn can_transition_to(&self, target: TaskState) -> bool {
        self.valid_transitions().contains(&target)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Backlog => write!(f, "BACKLOG"),
            TaskState::Queued => write!(f, "QUEUED"),
            TaskState::Research => write!(f, "RESEARCH"),
            TaskState::Plan => write!(f, "PLAN"),
            TaskState::Implement => write!(f, "IMPLEMENT"),
            TaskState::Validate => write!(f, "VALIDATE"),
            TaskState::Analyze => write!(f, "ANALYZE"),
            TaskState::AddNew => write!(f, "ADD_NEW"),
            TaskState::Migrate => write!(f, "MIGRATE"),
            TaskState::RemoveOld => write!(f, "REMOVE_OLD"),
            TaskState::Reproduce => write!(f, "REPRODUCE"),
            TaskState::Investigate => write!(f, "INVESTIGATE"),
            TaskState::Fix => write!(f, "FIX"),
            TaskState::Harden => write!(f, "HARDEN"),
            TaskState::Preparing => write!(f, "PREPARING"),
            TaskState::Spawning => write!(f, "SPAWNING"),
            TaskState::Prepare => write!(f, "PREPARE"),
            TaskState::Test => write!(f, "TEST"),
            TaskState::Execute => write!(f, "EXECUTE"),
            TaskState::Cleanup => write!(f, "CLEANUP"),
            TaskState::Review => write!(f, "REVIEW"),
            TaskState::Docs => write!(f, "DOCS"),
            TaskState::Learn => write!(f, "LEARN"),
            TaskState::Done => write!(f, "DONE"),
            TaskState::Failed => write!(f, "FAILED"),
        }
    }
}

impl TaskState {
    /// User-facing label for display in the TUI.
    /// Returns a concise, human-readable name instead of the internal ALL_CAPS form.
    pub fn user_label(self) -> &'static str {
        match self {
            TaskState::Backlog => "To Do",
            TaskState::Queued => "Queued",
            TaskState::Research => "Understanding",
            TaskState::Plan => "Planning",
            TaskState::Implement => "Implementing",
            TaskState::Validate => "Validating",
            TaskState::Analyze => "Analyzing",
            TaskState::AddNew => "Adding",
            TaskState::Migrate => "Migrating",
            TaskState::RemoveOld => "Removing",
            TaskState::Reproduce => "Reproducing",
            TaskState::Investigate => "Investigating",
            TaskState::Fix => "Fixing",
            TaskState::Harden => "Hardening",
            TaskState::Preparing => "Preparing",
            TaskState::Spawning => "Starting",
            TaskState::Prepare => "Preparing",
            TaskState::Test => "Testing",
            TaskState::Execute => "Executing",
            TaskState::Cleanup => "Cleaning up",
            TaskState::Review => "Reviewing",
            TaskState::Docs => "Documenting",
            TaskState::Learn => "Learning",
            TaskState::Done => "Done",
            TaskState::Failed => "Failed",
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BACKLOG" => Ok(TaskState::Backlog),
            "QUEUED" => Ok(TaskState::Queued),
            "RESEARCH" => Ok(TaskState::Research),
            "PLAN" => Ok(TaskState::Plan),
            "IMPLEMENT" => Ok(TaskState::Implement),
            "VALIDATE" => Ok(TaskState::Validate),
            "ANALYZE" => Ok(TaskState::Analyze),
            "ADD_NEW" => Ok(TaskState::AddNew),
            "MIGRATE" => Ok(TaskState::Migrate),
            "REMOVE_OLD" => Ok(TaskState::RemoveOld),
            "REPRODUCE" => Ok(TaskState::Reproduce),
            "INVESTIGATE" => Ok(TaskState::Investigate),
            "FIX" => Ok(TaskState::Fix),
            "HARDEN" => Ok(TaskState::Harden),
            "PREPARING" => Ok(TaskState::Preparing),
            "SPAWNING" => Ok(TaskState::Spawning),
            "PREPARE" => Ok(TaskState::Prepare),
            "TEST" => Ok(TaskState::Test),
            "EXECUTE" => Ok(TaskState::Execute),
            "CLEANUP" => Ok(TaskState::Cleanup),
            "REVIEW" => Ok(TaskState::Review),
            "DOCS" => Ok(TaskState::Docs),
            "LEARN" => Ok(TaskState::Learn),
            "DONE" => Ok(TaskState::Done),
            "FAILED" => Ok(TaskState::Failed),
            _ => Err(format!("Invalid task state: {}", s)),
        }
    }
}
