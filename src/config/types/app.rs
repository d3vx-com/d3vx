//! Top-level application configuration, UI, and budget settings

use serde::{Deserialize, Serialize};

use super::agent::AgentEnhancementsConfig;
use super::integrations::IntegrationsConfig;
use super::pipeline::PipelineConfig;
use super::providers::{ModelRouting, ProvidersConfig};
use super::recovery::RecoveryConfig;
use super::security::{PermissionsConfig, RoleConfig, SecurityConfig};
use super::subagent::SubAgentConfig;
use super::types_budget::BudgetConfig;
use super::types_claims::ClaimsConfig;
use super::types_features::FeatureFlagsConfig;
use super::types_git::GitConfig;
use super::types_hooks::HooksConfig;
use super::types_lsp::LspConfig;
use super::types_mcp::McpConfig;
use super::types_memory::MemoryConfig;
use super::types_notifications::NotificationsConfig;
use super::types_plugins::PluginsConfig;
use super::types_sandbox::SandboxConfig;
use super::types_structured_output::StructuredOutputConfig;
use super::types_ui::UiConfig;

/// UI mode selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    #[default]
    Chat,
    Kanban,
    List,
    Suggestion,
}

/// Complete d3vx configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct D3vxConfig {
    /// Config schema version
    #[serde(default = "default_version")]
    pub version: u32,
    /// Default LLM provider
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Default model
    #[serde(default = "default_model")]
    pub model: String,
    /// Provider configurations
    #[serde(default)]
    pub providers: ProvidersConfig,
    /// Model routing configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_routing: Option<ModelRouting>,
    /// Pipeline configuration
    pub pipeline: PipelineConfig,
    /// Permission configuration
    #[serde(default)]
    pub permissions: PermissionsConfig,
    /// Security configuration (command blocklists, etc.)
    #[serde(default)]
    pub security: SecurityConfig,
    /// Git integration configuration
    pub git: GitConfig,
    /// Plugin bindings configuration
    #[serde(default)]
    pub plugins: PluginsConfig,
    /// Memory configuration
    pub memory: MemoryConfig,
    /// MCP configuration
    #[serde(default)]
    pub mcp: McpConfig,
    /// Hooks configuration
    #[serde(default)]
    pub hooks: HooksConfig,
    /// UI configuration
    pub ui: UiConfig,
    /// Notifications configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<NotificationsConfig>,
    /// Budget guardrails configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetConfig>,
    /// Integrations configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrations: Option<IntegrationsConfig>,
    /// Role-based tool access configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<RoleConfig>,
    /// Error recovery configuration
    #[serde(default)]
    pub recovery: RecoveryConfig,
    /// Sub-agent configuration
    #[serde(default)]
    pub subagent: SubAgentConfig,
    /// Agent enhancements (compaction, doom loop, best-of-n, skills)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_enhancements: Option<AgentEnhancementsConfig>,
    /// LSP configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<LspConfig>,

    /// Structured output configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<StructuredOutputConfig>,
    /// Feature flags configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<FeatureFlagsConfig>,
    /// Sandbox configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// Claims-based authorization configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<ClaimsConfig>,
}

/// Partial configuration for loading from files/env
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct PartialD3vxConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub providers: Option<ProvidersConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_routing: Option<ModelRouting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<PipelineConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<NotificationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrations: Option<IntegrationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<RoleConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent: Option<SubAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_enhancements: Option<AgentEnhancementsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<LspConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<StructuredOutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<FeatureFlagsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// Claims-based authorization configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<ClaimsConfig>,
}

fn default_version() -> u32 {
    1
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}
