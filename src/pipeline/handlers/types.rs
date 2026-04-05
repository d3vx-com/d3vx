//! Phase handler types and trait definition

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

use crate::agent::{AgentLoop, AgentLoopError};
use crate::pipeline::phases::{Phase, PhaseContext, Task};

/// Errors that can occur during phase execution
#[derive(Debug, Error)]
pub enum PhaseError {
    /// The phase execution failed
    #[error("Phase execution failed: {message}")]
    ExecutionFailed { message: String },

    /// The phase was cancelled
    #[error("Phase was cancelled")]
    Cancelled,

    /// The phase timed out
    #[error("Phase timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Invalid state transition
    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// IO error during phase execution
    #[error("IO error: {source}")]
    IoError {
        #[source]
        source: std::io::Error,
    },

    /// Agent error during execution
    #[error("Agent error: {0}")]
    AgentError(#[from] AgentLoopError),

    /// No agent provided when required
    #[error("No agent provided for phase execution")]
    NoAgent,

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for PhaseError {
    fn from(err: std::io::Error) -> Self {
        PhaseError::IoError { source: err }
    }
}

/// Result of a phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    /// Whether the phase completed successfully
    pub success: bool,
    /// Output text from the phase
    pub output: String,
    /// Any errors that occurred
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Additional metadata from the execution
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    /// Files that were modified
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_modified: Vec<String>,
    /// Files that were created
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_created: Vec<String>,
    /// Git commit hash if a commit was made
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

impl PhaseResult {
    /// Create a successful result
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            errors: Vec::new(),
            metadata: serde_json::Value::Null,
            files_modified: Vec::new(),
            files_created: Vec::new(),
            commit_hash: None,
        }
    }

    /// Create a failed result
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            errors: vec![message.into()],
            metadata: serde_json::Value::Null,
            files_modified: Vec::new(),
            files_created: Vec::new(),
            commit_hash: None,
        }
    }

    /// Add an error to the result
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    /// Add metadata to the result
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add modified file to the result
    pub fn with_modified_file(mut self, path: impl Into<String>) -> Self {
        self.files_modified.push(path.into());
        self
    }

    /// Add created file to the result
    pub fn with_created_file(mut self, path: impl Into<String>) -> Self {
        self.files_created.push(path.into());
        self
    }

    /// Set the commit hash
    pub fn with_commit(mut self, hash: impl Into<String>) -> Self {
        self.commit_hash = Some(hash.into());
        self
    }

    /// Check if the result has any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl fmt::Display for PhaseResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "Phase completed successfully")
        } else {
            write!(f, "Phase failed: {}", self.errors.join(", "))
        }
    }
}

/// Trait for phase handlers
#[async_trait]
pub trait PhaseHandler: Send + Sync {
    /// Get the phase this handler is for
    fn phase(&self) -> Phase;

    /// Execute the phase with an optional agent
    ///
    /// The agent is optional to support dry-run and testing scenarios.
    /// In production, an agent should always be provided.
    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError>;

    /// Get a human-readable name for this handler
    fn name(&self) -> &'static str {
        self.phase().label()
    }

    /// Validate that the task can be executed by this handler
    fn can_execute(&self, task: &Task) -> Result<(), PhaseError> {
        if task.phase != self.phase() {
            return Err(PhaseError::InvalidTransition {
                from: task.phase.to_string(),
                to: self.phase().to_string(),
            });
        }
        Ok(())
    }
}
