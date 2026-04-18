//! Data shape of a coordination task.
//!
//! Kept separate from [`board`](super::board) so the struct definitions
//! and small helpers don't pull in filesystem concerns; helps testing
//! and keeps each file focused.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A task on the coordination board.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoardTask {
    pub id: String,
    pub title: String,
    pub instruction: String,
    pub status: TaskStatus,
    /// Agent id currently owning this task, or `None` if unclaimed.
    pub owner: Option<String>,
    /// Task ids that must reach `Completed` before this task is ready.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Populated when the task reaches `Completed` or `Failed`.
    #[serde(default)]
    pub result: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }
}

/// Input for adding a new task to the board.
#[derive(Debug, Clone)]
pub struct NewTask {
    pub id: String,
    pub title: String,
    pub instruction: String,
    pub depends_on: Vec<String>,
}

impl NewTask {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        instruction: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            instruction: instruction.into(),
            depends_on: Vec::new(),
        }
    }

    pub fn with_depends_on(mut self, ids: Vec<String>) -> Self {
        self.depends_on = ids;
        self
    }
}
