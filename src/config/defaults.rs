//! Default configuration values for d3vx
//!
//! These defaults are used when no configuration is provided.

use super::security::{BashToolConfig, SecurityConfig};
use super::types::*;
use std::collections::HashMap;

/// Environment variable mapping to config paths
pub const ENV_VAR_MAP: &[(&str, &str)] = &[
    ("D3VX_PROVIDER", "provider"),
    ("D3VX_MODEL", "model"),
    ("D3VX_TRUST_MODE", "permissions.trust_mode"),
    ("D3VX_AUTO_COMMIT", "git.auto_commit"),
    ("D3VX_AUTO_PUSH", "git.auto_push"),
    ("D3VX_MEMORY_ENABLED", "memory.enabled"),
    ("D3VX_MAX_ENTRIES", "memory.max_entries"),
    ("D3VX_UI_MODE", "ui.mode"),
    ("D3VX_UI_AUTONOMOUS", "ui.autonomous"),
    ("D3VX_UI_FLOATING_STATUS", "ui.floating_status"),
    ("D3VX_UI_POWER_MODE", "ui.power_mode"),
    ("D3VX_UI_SHOW_WELCOME", "ui.show_welcome"),
    ("D3VX_UI_SIDEBAR_WIDTH", "ui.sidebar_width"),
    // Agent Enhancements
    (
        "D3VX_COMPACTION_ENABLED",
        "agent_enhancements.compaction.enabled",
    ),
    (
        "D3VX_DOOM_LOOP_ENABLED",
        "agent_enhancements.doom_loop.enabled",
    ),
    ("D3VX_BEST_OF_N", "agent_enhancements.best_of_n.n"),
    ("D3VX_SKILLS_ENABLED", "agent_enhancements.skills.enabled"),
    // LSP
    ("D3VX_LSP_ENABLED", "lsp.enabled"),
    ("D3VX_LSP_DIAGNOSTICS", "lsp.enable_diagnostics"),
    // Structured Output
    (
        "D3VX_STRUCTURED_OUTPUT_ENABLED",
        "structured_output.enabled",
    ),
    // Sub-agent
    (
        "D3VX_SUBAGENT_RETENTION",
        "subagent.cleanup.retention_period_secs",
    ),
];

/// Get the global config directory path
pub fn get_global_config_dir() -> String {
    // Check for ~/.d3vx first (common for developer tools)
    let home = dirs::home_dir().map(|p| p.join(".d3vx"));
    if let Some(ref path) = home {
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    // Fallback to OS-specific config dir
    dirs::config_dir()
        .map(|p| p.join("d3vx"))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.d3vx".to_string())
}

/// Get the global config file path
pub fn get_global_config_path() -> String {
    format!("{}/config.yml", get_global_config_dir())
}

/// Get the global environment file path
pub fn get_global_env_path() -> String {
    format!("{}/.env", get_global_config_dir())
}

/// Get the project config directory path
pub fn get_project_config_dir(project_root: &str) -> String {
    format!("{}/.d3vx", project_root)
}

/// Get the project config file path
pub fn get_project_config_path(project_root: &str) -> String {
    format!("{}/config.yml", get_project_config_dir(project_root))
}

/// Create default pipeline phases
fn default_pipeline_phases() -> PipelinePhases {
    PipelinePhases {
        research: PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        plan: PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        implement: PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        review: PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
        docs: PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
        learn: PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
    }
}

/// Create default provider configs from the registry
fn default_provider_configs() -> HashMap<String, ProviderConfig> {
    use crate::providers::SUPPORTED_PROVIDERS;

    let mut configs = HashMap::new();

    for provider in SUPPORTED_PROVIDERS.all() {
        configs.insert(
            provider.id.to_string(),
            ProviderConfig {
                api_key_env: if provider.api_key_env.is_empty() {
                    None
                } else {
                    Some(provider.api_key_env.to_string())
                },
                default_model: provider.default_model.to_string(),
                base_url: provider.base_url.map(String::from),
                research_model: provider.cheap_model.map(String::from),
                cheap_model: None,
                timeout_ms: Some(provider.timeout_ms),
                max_retries: Some(if provider.id == "ollama" { 1 } else { 3 }),
            },
        );
    }

    configs
}

/// The default d3vx configuration
pub fn default_config() -> D3vxConfig {
    D3vxConfig {
        version: 1,
        provider: "anthropic".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        providers: ProvidersConfig {
            fallback_chain: None,
            configs: Some(default_provider_configs()),
        },
        model_routing: None,
        pipeline: PipelineConfig {
            phases: default_pipeline_phases(),
            stop_on_failure: true,
            checkpoint: true,
            budget: None,
            timeouts: Some(PipelineTimeouts {
                pipeline_minutes: 60,
                phase_minutes: 20,
            }),
            max_concurrent_agents: 3,
        },
        permissions: PermissionsConfig {
            auto_approve: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string()],
            require_approval: vec!["Bash".to_string(), "Write".to_string(), "Edit".to_string()],
            allow: vec![
                "Read(**)".to_string(),
                "Glob(**)".to_string(),
                "Grep(**)".to_string(),
                "Bash(git:*)".to_string(),
                "Bash(npm:*)".to_string(),
                "Bash(node:*)".to_string(),
                "Edit(src/**)".to_string(),
                "mcp__*".to_string(),
            ],
            deny: vec![
                "Read(**/.env*)".to_string(),
                "Read(**/credentials/**)".to_string(),
                "Read(~/.ssh/**)".to_string(),
                "Write(/etc/**)".to_string(),
                "Bash(rm:-rf*)".to_string(),
                "Bash(sudo:*)".to_string(),
            ],
            ask: vec![],
            deny_always: None,
            trust_mode: false,
        },
        security: SecurityConfig {
            bash_tool: BashToolConfig { blocklist: vec![] },
        },
        git: GitConfig {
            auto_commit: true,
            auto_push: false,
            worktree_dir: ".d3vx-worktrees".to_string(),
            commit_prefix: "feat".to_string(),
            ai_commit_messages: true,
            commit_message_max_tokens: 100,
            main_branch: "main".to_string(),
            sign_commits: false,
            pre_commit_hooks: PreCommitConfig::default(),
        },
        plugins: PluginsConfig::default(),
        memory: MemoryConfig {
            enabled: true,
            dir: ".d3vx/memory".to_string(),
            max_entries: 10000,
            auto_learn: true,
            enable_search: true,
        },
        mcp: McpConfig {
            servers: HashMap::new(),
        },
        hooks: HooksConfig::default(),
        ui: UiConfig {
            mode: UiMode::Chat,
            autonomous: false,
            floating_status: true,
            auto_switch_on_tasks: false,
            refresh_interval_ms: 1000,
            show_help_footer: true,
            power_mode: false,
            show_welcome: true,
            sidebar_width: 30,
        },
        notifications: None,
        budget: Some(BudgetConfig {
            per_session: 5.00,
            per_day: 50.00,
            warn_at: 0.8,
            pause_at: 1.0,
            enabled: true,
        }),
        integrations: Some(IntegrationsConfig {
            tracker: TrackerType::Github,
            github: Some(GitHubIntegration {
                repository: None,
                token_env: "GITHUB_TOKEN".to_string(),
                api_base_url: "https://api.github.com".to_string(),
                default_branch: "main".to_string(),
                use_cli: false,
                auto_create_issues_for_manual_tasks: false,
                auto_raise_prs: false,
            }),
            linear: None,
            webhooks: Some(WebhooksConfig {
                endpoints: vec![],
                secret: None,
                logging: true,
                log_retention_days: 30,
            }),
            github_actions: Some(GitHubActionsConfig {
                enabled: false,
                context_prefix: "d3vx".to_string(),
                use_check_runs: false,
                dashboard_url: None,
                on_task_started: true,
                on_task_completed: true,
                on_task_failed: true,
            }),
        }),
        roles: None,
        recovery: crate::config::types::RecoveryConfig::default(),
        subagent: Default::default(),
        agent_enhancements: None,
        lsp: None,
        structured_output: None,
        features: None,
        sandbox: None,
        claims: None,
    }
}

/// Lazy static for default config
pub static DEFAULT_CONFIG: once_cell::sync::Lazy<D3vxConfig> =
    once_cell::sync::Lazy::new(default_config);
