//! Common error types for d3vx
//!
//! This module provides standardized error types for common operations,
//! reducing the need for `.unwrap()` and improving error handling.

use std::path::PathBuf;
use thiserror::Error;

/// Common result type alias using D3vxError
pub type Result<T> = std::result::Result<T, D3vxError>;

/// Unified error type for d3vx
#[derive(Debug, Error)]
pub enum D3vxError {
    // File operations
    #[error("Failed to read file {path}: {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file {path}: {source}")]
    FileWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    // Configuration
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid configuration at {path}: {message}")]
    ConfigInvalid { path: PathBuf, message: String },

    // JSON/YAML parsing
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(String),

    #[error("Invalid JSON: {0}")]
    JsonInvalid(String),

    // Database
    #[error("Database error: {0}")]
    Database(String),

    #[error("Database not found: {0}")]
    DatabaseNotFound(PathBuf),

    // Agent
    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Agent timeout after {seconds}s")]
    AgentTimeout { seconds: u64 },

    #[error("Agent crashed: {0}")]
    AgentCrashed(String),

    // Provider
    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Provider unavailable: {name}")]
    ProviderUnavailable { name: String },

    #[error("Rate limited by provider {name}, retry after {retry_after}s")]
    RateLimited { name: String, retry_after: u64 },

    // Tool
    #[error("Tool not found: {name}")]
    ToolNotFound { name: String },

    #[error("Tool execution failed: {name} - {reason}")]
    ToolFailed { name: String, reason: String },

    #[error("Permission denied for tool: {name}")]
    ToolPermissionDenied { name: String },

    // Session
    #[error("Session not found: {id}")]
    SessionNotFound { id: String },

    #[error("Session expired: {id}")]
    SessionExpired { id: String },

    // Task
    #[error("Task not found: {id}")]
    TaskNotFound { id: String },

    #[error("Task failed: {id} - {reason}")]
    TaskFailed { id: String, reason: String },

    #[error("Invalid task state transition: {from} -> {to}")]
    InvalidTaskTransition { from: String, to: String },

    // Worktree
    #[error("Worktree error: {0}")]
    Worktree(String),

    #[error("Worktree not found: {name}")]
    WorktreeNotFound { name: String },

    #[error("Worktree conflict: {name}")]
    WorktreeConflict { name: String },

    // Validation
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid input: {field} - {reason}")]
    InvalidInput { field: String, reason: String },

    // Internal
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    // Permission
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl D3vxError {
    /// Check if this error should cause a retry
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            D3vxError::RateLimited { .. }
                | D3vxError::ProviderUnavailable { .. }
                | D3vxError::FileRead { .. }
                | D3vxError::Database(..)
        )
    }

    /// Check if this error indicates a bug
    pub fn is_bug(&self) -> bool {
        matches!(
            self,
            D3vxError::Internal(..)
                | D3vxError::NotImplemented(..)
                | D3vxError::InvalidTaskTransition { .. }
        )
    }

    /// Get a user-friendly message
    pub fn user_message(&self) -> String {
        match self {
            D3vxError::FileNotFound { path } => {
                format!("File not found: {}", path.display())
            }
            D3vxError::ToolNotFound { name } => {
                format!("Tool '{}' not found", name)
            }
            D3vxError::TaskFailed { id, reason } => {
                format!("Task {} failed: {}", id, reason)
            }
            D3vxError::AgentTimeout { seconds } => {
                format!("Operation timed out after {} seconds", seconds)
            }
            _ => self.to_string(),
        }
    }
}

// ────────────────────────────────────────────────────────────
// Extension traits for converting common types
// ────────────────────────────────────────────────────────────

/// Extension trait for Option to convert None to D3vxError
pub trait OptionExt<T> {
    fn context(self, msg: impl Into<String>) -> Result<T>;
    fn with_context<C: Into<String>>(self, context: C) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.ok_or_else(|| D3vxError::Internal(msg.into()))
    }

    fn with_context<C: Into<String>>(self, context: C) -> Result<T> {
        self.context(context)
    }
}

/// Extension trait for Result to add context
pub trait ResultExt<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T>;
    fn map_err_context<F: FnOnce(E) -> String>(self, f: F) -> Result<T>;
}

impl<T, E: std::fmt::Debug> ResultExt<T, E> for std::result::Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|_| D3vxError::Internal(msg.into()))
    }

    fn map_err_context<F: FnOnce(E) -> String>(self, f: F) -> Result<T> {
        self.map_err(|e| D3vxError::Internal(f(e)))
    }
}

// ────────────────────────────────────────────────────────────
// Validation helpers
// ────────────────────────────────────────────────────────────

/// Validate that a path exists
pub fn validate_path_exists(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(D3vxError::FileNotFound { path: path.clone() });
    }
    Ok(())
}

/// Validate that a string is not empty
pub fn validate_not_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(D3vxError::InvalidInput {
            field: field.to_string(),
            reason: "cannot be empty".to_string(),
        });
    }
    Ok(())
}

/// Validate that a number is in range
pub fn validate_range(field: &str, value: u64, min: u64, max: u64) -> Result<()> {
    if value < min || value > max {
        return Err(D3vxError::InvalidInput {
            field: field.to_string(),
            reason: format!("must be between {} and {}", min, max),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_context() {
        let value: Option<i32> = None;
        let result: Result<i32> = value.context("value was None");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_not_empty() {
        assert!(validate_not_empty("name", "test").is_ok());
        assert!(validate_not_empty("name", "").is_err());
        assert!(validate_not_empty("name", "   ").is_err());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range("age", 25, 0, 100).is_ok());
        assert!(validate_range("age", 150, 0, 100).is_err());
        assert!(validate_range("age", 0, 0, 100).is_ok());
    }

    #[test]
    fn test_error_messages() {
        let err = D3vxError::FileNotFound {
            path: PathBuf::from("/tmp/test.txt"),
        };
        assert!(err.to_string().contains("test.txt"));

        let err = D3vxError::ToolNotFound {
            name: "Read".to_string(),
        };
        assert!(err.user_message().contains("Read"));
    }
}
