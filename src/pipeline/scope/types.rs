//! Scope Types
//!
//! Data structures for task scope handling.

use std::path::PathBuf;

/// How the task scope relates to the repository
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeMode {
    /// Task operates on entire repository
    Repo,
    /// Task is limited to a subdirectory
    Subdir,
    /// Task operates on a nested git repository within a parent
    NestedRepo,
    /// Task spans multiple repositories (parent task with children)
    MultiRepo,
}

impl std::fmt::Display for ScopeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeMode::Repo => write!(f, "repo"),
            ScopeMode::Subdir => write!(f, "subdir"),
            ScopeMode::NestedRepo => write!(f, "nested_repo"),
            ScopeMode::MultiRepo => write!(f, "multi_repo"),
        }
    }
}

impl Default for ScopeMode {
    fn default() -> Self {
        ScopeMode::Repo
    }
}

/// Errors in scope handling
#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    /// Path is invalid
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Path is outside the repository
    #[error("Path is outside the repository: {0}")]
    PathOutsideRepo(PathBuf),

    /// Path is outside the task scope
    #[error("Path is outside the task scope: {0}")]
    PathOutsideScope(PathBuf),

    /// Scope expansion not allowed
    #[error("Scope expansion is not allowed for this task")]
    ExpansionNotAllowed,

    /// No git repository found
    #[error("No git repository found at or above: {0}")]
    NoRepoFound(PathBuf),

    /// Workspace error
    #[error("Workspace error: {0}")]
    WorkspaceError(String),
}
