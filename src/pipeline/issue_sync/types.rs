//! Issue Sync Types
//!
//! Data types for bidirectional issue tracker synchronization.

use serde::{Deserialize, Serialize};

/// Issue from an external tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalIssue {
    pub id: String,
    pub number: Option<u64>,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub url: Option<String>,
    pub tracker: TrackerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    InProgress,
    Closed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackerKind {
    Github,
    Linear,
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub issues_fetched: usize,
    pub tasks_created: usize,
    pub tasks_updated: usize,
    pub errors: Vec<String>,
}

impl Default for SyncResult {
    fn default() -> Self {
        Self {
            issues_fetched: 0,
            tasks_created: 0,
            tasks_updated: 0,
            errors: Vec::new(),
        }
    }
}

impl SyncResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_errors(errors: Vec<String>) -> Self {
        Self {
            errors,
            ..Self::default()
        }
    }
}

/// Errors that can occur during sync operations.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Tracker unavailable: {0}")]
    Unavailable(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Not configured")]
    NotConfigured,
}
