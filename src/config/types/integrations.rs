//! Integration configurations (GitHub, Linear, webhooks, notifications)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Issue tracker type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrackerType {
    #[default]
    Github,
    Linear,
    None,
}

/// Linear integration configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LinearIntegration {
    /// API key for Linear
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Environment variable name containing the API key
    #[serde(default = "default_linear_api_key_env")]
    pub api_key_env: String,
    /// Team ID to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// Workflow state IDs for status mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_states: Option<LinearWorkflowStates>,
    /// Request timeout in milliseconds
    #[serde(default = "default_linear_timeout")]
    pub timeout_ms: u64,
}

/// Linear workflow state IDs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LinearWorkflowStates {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backlog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_progress: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled: Option<String>,
}

/// GitHub integration configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GitHubIntegration {
    /// Repository in owner/repo format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Environment variable containing a GitHub token
    #[serde(default = "default_github_token_env")]
    pub token_env: String,
    /// GitHub API base URL
    #[serde(default = "default_github_api_base_url")]
    pub api_base_url: String,
    /// Default branch
    #[serde(default = "default_main_branch")]
    pub default_branch: String,
    /// Use gh CLI for operations
    #[serde(default = "default_true")]
    pub use_cli: bool,
    /// Automatically create GitHub issues for manual background tasks
    #[serde(default)]
    pub auto_create_issues_for_manual_tasks: bool,
    /// Automatically raise PRs for completed autonomous tasks
    #[serde(default)]
    pub auto_raise_prs: bool,
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct WebhookConfig {
    /// Unique identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Webhook endpoint URL
    pub url: String,
    /// Secret for HMAC signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Events to subscribe to
    pub events: Vec<String>,
    /// Whether this webhook is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Webhooks configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct WebhooksConfig {
    /// List of webhook configurations
    #[serde(default)]
    pub endpoints: Vec<WebhookConfig>,
    /// Global secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Enable webhook delivery logging
    #[serde(default = "default_true")]
    pub logging: bool,
    /// Days to keep delivery logs
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,
}

/// GitHub Actions integration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GitHubActionsConfig {
    /// Enable GitHub status updates
    #[serde(default)]
    pub enabled: bool,
    /// Context prefix for status updates
    #[serde(default = "default_context_prefix")]
    pub context_prefix: String,
    /// Use Check Runs instead of simple status
    #[serde(default)]
    pub use_check_runs: bool,
    /// Dashboard URL for target links
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_url: Option<String>,
    /// Report task started events
    #[serde(default = "default_true")]
    pub on_task_started: bool,
    /// Report task completed events
    #[serde(default = "default_true")]
    pub on_task_completed: bool,
    /// Report task failed events
    #[serde(default = "default_true")]
    pub on_task_failed: bool,
}

/// Top-level integrations configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct IntegrationsConfig {
    /// Issue tracker type
    #[serde(default)]
    pub tracker: TrackerType,
    /// GitHub integration settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GitHubIntegration>,
    /// Linear integration settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear: Option<LinearIntegration>,
    /// Webhook configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<WebhooksConfig>,
    /// GitHub Actions integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_actions: Option<GitHubActionsConfig>,
}

fn default_true() -> bool {
    true
}

fn default_github_token_env() -> String {
    "GITHUB_TOKEN".to_string()
}

fn default_github_api_base_url() -> String {
    "https://api.github.com".to_string()
}

fn default_main_branch() -> String {
    "main".to_string()
}

fn default_linear_api_key_env() -> String {
    "LINEAR_API_KEY".to_string()
}

fn default_linear_timeout() -> u64 {
    30000
}

fn default_context_prefix() -> String {
    "d3vx".to_string()
}

fn default_log_retention_days() -> u32 {
    30
}
