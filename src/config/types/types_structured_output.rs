//! Structured output configuration

use serde::{Deserialize, Serialize};

/// Configuration for structured output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct StructuredOutputConfig {
    /// Enable structured output support
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default schema to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_schema: Option<serde_json::Value>,
    /// Maximum retry attempts for validation
    #[serde(default = "default_structured_retries")]
    pub max_retries: usize,
    /// Strict schema validation
    #[serde(default = "default_true")]
    pub strict_validation: bool,
}

fn default_true() -> bool {
    true
}

fn default_structured_retries() -> usize {
    3
}
