//! Commander Validation Types
//!
//! Data types for the validation runner: kinds, commands, and results.

use serde::{Deserialize, Serialize};

/// Kind of validation being performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationKind {
    TypeCheck,
    Test,
    Lint,
    Custom(String),
}

impl std::fmt::Display for ValidationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationKind::TypeCheck => write!(f, "type_check"),
            ValidationKind::Test => write!(f, "test"),
            ValidationKind::Lint => write!(f, "lint"),
            ValidationKind::Custom(name) => write!(f, "custom({name})"),
        }
    }
}

/// The result of running a single validation command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub kind: ValidationKind,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// A validation command to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCommand {
    pub kind: ValidationKind,
    pub command: String,
    pub timeout_secs: u64,
}

impl Default for ValidationCommand {
    fn default() -> Self {
        Self {
            kind: ValidationKind::TypeCheck,
            command: "cargo check".to_string(),
            timeout_secs: 120,
        }
    }
}

impl ValidationCommand {
    /// Create a cargo check command.
    pub fn type_check() -> Self {
        Self {
            kind: ValidationKind::TypeCheck,
            command: "cargo check".to_string(),
            timeout_secs: 120,
        }
    }

    /// Create a cargo test command.
    pub fn test() -> Self {
        Self {
            kind: ValidationKind::Test,
            command: "cargo test".to_string(),
            timeout_secs: 300,
        }
    }

    /// Create a cargo clippy command.
    pub fn lint() -> Self {
        Self {
            kind: ValidationKind::Lint,
            command: "cargo clippy".to_string(),
            timeout_secs: 120,
        }
    }

    /// Create a custom validation command.
    pub fn custom(name: &str, command: &str, timeout_secs: u64) -> Self {
        Self {
            kind: ValidationKind::Custom(name.to_string()),
            command: command.to_string(),
            timeout_secs,
        }
    }
}
