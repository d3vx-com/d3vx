//! LSP configuration

use serde::{Deserialize, Serialize};

/// LSP server configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LspServer {
    /// Command to start the server
    pub command: Vec<String>,
    /// Arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// File extensions this server handles
    pub extensions: Vec<String>,
    /// Initialization options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
}

/// LSP configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LspConfig {
    /// Enable LSP features
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// LSP server configurations
    #[serde(default)]
    pub servers: std::collections::HashMap<String, LspServer>,
    /// Enable diagnostics
    #[serde(default = "default_true")]
    pub enable_diagnostics: bool,
    /// Enable completion
    #[serde(default = "default_true")]
    pub enable_completion: bool,
    /// Enable go-to-definition
    #[serde(default = "default_true")]
    pub enable_goto: bool,
    /// Debounce interval for diagnostics (ms)
    #[serde(default = "default_diagnostic_debounce")]
    pub diagnostic_debounce_ms: u64,
}

fn default_true() -> bool {
    true
}

fn default_diagnostic_debounce() -> u64 {
    300
}
