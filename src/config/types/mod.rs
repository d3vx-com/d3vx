//! Configuration type definitions for d3vx
//!
//! These types map directly to the YAML configuration schema.

// Submodules
pub mod agent;
pub mod app;
pub mod integrations;
pub mod pipeline;
pub mod providers;
pub mod recovery;
pub mod security;
pub mod subagent;
pub mod types_budget;
pub mod types_claims;
pub mod types_features;
pub mod types_git;
pub mod types_hooks;
pub mod types_lsp;
pub mod types_mcp;
pub mod types_memory;
pub mod types_notifications;
pub mod types_plugins;
pub mod types_sandbox;
pub mod types_structured_output;
pub mod types_ui;

#[cfg(test)]
mod tests_defaults;
#[cfg(test)]
mod tests_flags_features;
#[cfg(test)]
mod tests_types;
#[cfg(test)]
mod tests_types2;

// Re-export all types for backward compatibility
pub use agent::{
    AgentEnhancementsConfig, BestOfNSettings, CompactionSettings, DoomLoopSettings, SkillsSettings,
};
pub use app::{D3vxConfig, PartialD3vxConfig, UiMode};
pub use integrations::{
    GitHubActionsConfig, GitHubIntegration, IntegrationsConfig, LinearIntegration,
    LinearWorkflowStates, TrackerType, WebhookConfig, WebhooksConfig,
};
pub use pipeline::{
    PipelineBudget, PipelineConfig, PipelinePhase, PipelinePhases, PipelineTimeouts,
};
pub use providers::{ModelRouting, ProviderConfig, ProvidersConfig};
pub use recovery::RecoveryConfig;
pub use security::{
    BashToolConfig, PermissionsConfig, RoleConfig, RoleToolPermissions, SecurityConfig,
};
pub use subagent::{CleanupConfig, SubAgentConfig};
pub use types_budget::BudgetConfig;
pub use types_claims::{Claim, ClaimsConfig};
pub use types_features::FeatureFlagsConfig;
pub use types_git::{GitConfig, PreCommitConfig};
pub use types_hooks::{Hook, HookEvent, HookType, HooksConfig};
pub use types_lsp::{LspConfig, LspServer};
pub use types_mcp::{McpConfig, McpServer};
pub use types_memory::MemoryConfig;
pub use types_notifications::{NotificationsConfig, TelegramConfig};
pub use types_plugins::{
    PluginCapability, PluginDiscovery, PluginEnabled, PluginManifest, PluginSetting, PluginsConfig,
};
pub use types_sandbox::{FilesystemRestriction, NetworkRestriction, SandboxConfig, SandboxMode};
pub use types_structured_output::StructuredOutputConfig;
pub use types_ui::UiConfig;
