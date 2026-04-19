//! Single error type for the coordination layer.
//!
//! One enum keeps callers from having to pattern-match N different error
//! types as they chain board + inbox + IO operations. Every error carries
//! enough context (paths, ids, reasons) that an operator reading a log can
//! reconstruct what went wrong without re-running.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoordinationError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize JSON for {path}: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to parse JSON at {path}: {source}")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("task `{task_id}` does not exist on this board")]
    TaskNotFound { task_id: String },

    #[error("task `{task_id}` is already claimed by `{owner}`")]
    AlreadyClaimed { task_id: String, owner: String },

    #[error(
        "task `{task_id}` cannot transition from `{from:?}` to `{to:?}`"
    )]
    InvalidTransition {
        task_id: String,
        from: super::board::TaskStatus,
        to: super::board::TaskStatus,
    },

    #[error(
        "task `{task_id}` is not yet ready (depends on: {depends_on:?} — \
         unresolved: {unresolved:?})"
    )]
    NotReady {
        task_id: String,
        depends_on: Vec<String>,
        unresolved: Vec<String>,
    },
}
