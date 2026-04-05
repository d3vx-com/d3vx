//! Task store for pipeline task persistence
//!
//! Handles the kanban state machine for task management,
//! including state transitions, dependency checks, and audit logging.

mod crud;
pub mod enums;
mod events;
mod owned;
mod queries;
pub mod state_machine;
pub mod types;

#[cfg(test)]
mod tests_crud;

#[cfg(test)]
mod tests_queries;

#[cfg(test)]
mod tests_state_machine;

use rusqlite::Connection;

use super::database::Database;

pub use enums::{AgentRole, ExecutionMode};
pub use state_machine::TaskState;
pub use types::{NewTask, Task, TaskListOptions, TaskLog, TaskUpdate};

/// Task store for CRUD operations
pub struct TaskStore<'a> {
    pub(super) conn: &'a Connection,
}

impl<'a> TaskStore<'a> {
    /// Create a new task store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new task store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }
}
