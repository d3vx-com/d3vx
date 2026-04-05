//! Hooks configuration

use serde::{Deserialize, Serialize};

/// Individual hook configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub struct Hook {
    /// Type of hook: command or mcp
    #[serde(rename = "type")]
    pub hook_type: HookType,
    /// Command to execute (for command type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// MCP server (for mcp type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// MCP tool (for mcp type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Timeout in seconds
    #[serde(default = "default_hook_timeout")]
    pub timeout: u64,
}

/// Hook type enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HookType {
    Command,
    Mcp,
}

/// Hook event configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookEvent {
    /// Matcher pattern for the hook
    #[serde(default = "default_matcher")]
    pub matcher: String,
    /// Hooks to execute
    pub hooks: Vec<Hook>,
}

/// Top-level hooks configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "PascalCase")]
pub struct HooksConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_start: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_prompt_submit: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_tool_use: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_tool_use: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_stop: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<HookEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<Vec<HookEvent>>,
}

fn default_hook_timeout() -> u64 {
    30
}

fn default_matcher() -> String {
    "*".to_string()
}
