//! Persistence layer for d3vx
//!
//! This module provides SQLite-based storage for sessions, tasks, messages,
//! and memory entries. It uses rusqlite for database operations with WAL mode
//! for concurrent read/write support.
//!
//! # Modules
//!
//! - `database`: Database connection and migration management
//! - `session`: Session CRUD operations
//! - `message`: Message store for conversations
//! - `task`: Task and task log persistence
//! - `migrations`: SQL schema migrations
//! - `run`: Task run tracking for execution attempts
//! - `workspace`: Isolated execution environment management
//! - `worker`: Agent process management
//! - `event`: Append-only event logging
//! - `history`: Unified history reader for sessions and events

pub mod database;
pub mod event;
pub mod history;
pub mod message;
pub mod migrations;
pub mod run;
pub mod session;
pub mod task;
pub mod tool_execution;
pub mod tool_permissions;
pub mod worker;
pub mod workspace;

// Re-export tool permissions store
pub use tool_permissions::ToolPermissionStore;
// Re-export tool execution audit store
pub use tool_execution::{NewToolExecution, ToolExecutionRecord, ToolExecutionStore};
// Re-export commonly used types from database and session
pub use database::{Database, DatabaseError};
pub use message::{MessageRecord, MessageRole, MessageStore};
pub use session::{Session, SessionListOptions, SessionStore};

// Re-export task types (core task types are in task.rs)
pub use task::{AgentRole, ExecutionMode};
pub use task::{NewTask, Task, TaskListOptions, TaskLog, TaskState, TaskStore, TaskUpdate};

// Re-export run types
pub use run::{NewTaskRun, RunStatus, TaskRun, TaskRunListOptions, TaskRunStore, TaskRunUpdate};

// Re-export workspace types
pub use workspace::{
    NewWorkspace, ScopeMode, Workspace, WorkspaceListOptions, WorkspaceStatus, WorkspaceStore,
    WorkspaceType,
};

// Re-export worker types
pub use worker::{
    RegisterWorker, Worker, WorkerListOptions, WorkerStatus, WorkerStore, WorkerType,
};

// Re-export event types
pub use event::{EventListOptions, EventStore, EventType, NewEvent, TaskEvent};

// Re-export history types
pub use history::reader::{
    HistoryBounds, HistoryFilter, HistoryKind, HistoryQuery, HistoryReader, HistoryResult,
    HistoryStats,
};
pub use history::transcript::{
    TranscriptEntry, TranscriptReader, TranscriptRole, TranscriptSummary,
};

/// Generate a unique ID for database records
pub fn generate_id(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_millis(0))
        .as_millis();
    let random: u32 = rand::random();
    format!("{}-{:x}-{:06x}", prefix, timestamp, random & 0xFFFFFF)
}

/// Get current ISO timestamp
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = generate_id("ses");
        let id2 = generate_id("ses");

        assert!(id1.starts_with("ses-"));
        assert!(id2.starts_with("ses-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_now_iso() {
        let ts = now_iso();
        assert!(ts.contains('T'));
        assert!(ts.contains('Z') || ts.contains('+'));
    }
}
