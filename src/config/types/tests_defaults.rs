//! Tests for config defaults module

use super::UiMode;
use crate::config::defaults::{
    default_config, get_project_config_dir, get_project_config_path, ENV_VAR_MAP,
};

#[test]
fn test_default_config_is_complete() {
    let config = default_config();
    assert_eq!(config.version, 1);
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_default_config_pipeline_phases_all_enabled() {
    let config = default_config();
    assert!(config.pipeline.phases.research.enabled);
    assert!(config.pipeline.phases.plan.enabled);
    assert!(config.pipeline.phases.implement.enabled);
    assert!(config.pipeline.phases.review.enabled);
    assert!(config.pipeline.phases.docs.enabled);
    assert!(config.pipeline.phases.learn.enabled);
}

#[test]
fn test_default_config_pipeline_retries() {
    let config = default_config();
    assert_eq!(config.pipeline.phases.research.max_retries, 3);
    assert_eq!(config.pipeline.phases.plan.max_retries, 3);
    assert_eq!(config.pipeline.phases.implement.max_retries, 3);
    assert_eq!(config.pipeline.phases.review.max_retries, 1);
    assert_eq!(config.pipeline.phases.docs.max_retries, 1);
    assert_eq!(config.pipeline.phases.learn.max_retries, 1);
}

#[test]
fn test_default_config_pipeline_timeouts() {
    let config = default_config();
    let timeouts = config.pipeline.timeouts.as_ref().unwrap();
    assert_eq!(timeouts.pipeline_minutes, 60);
    assert_eq!(timeouts.phase_minutes, 20);
}

#[test]
fn test_default_config_pipeline_concurrency() {
    let config = default_config();
    assert_eq!(config.pipeline.max_concurrent_agents, 3);
}

#[test]
fn test_default_config_permissions() {
    let config = default_config();
    assert!(config
        .permissions
        .auto_approve
        .contains(&"Read".to_string()));
    assert!(config
        .permissions
        .require_approval
        .contains(&"Bash".to_string()));
    assert!(!config.permissions.trust_mode);
}

#[test]
fn test_default_config_git() {
    let config = default_config();
    assert!(config.git.auto_commit);
    assert!(!config.git.auto_push);
    assert_eq!(config.git.worktree_dir, ".d3vx-worktrees");
    assert_eq!(config.git.main_branch, "main");
}

#[test]
fn test_default_config_ui() {
    let config = default_config();
    assert!(matches!(config.ui.mode, UiMode::Chat));
    assert!(!config.ui.autonomous);
    assert!(config.ui.floating_status);
    assert!(config.ui.show_welcome);
    assert_eq!(config.ui.sidebar_width, 30);
}

#[test]
fn test_default_config_memory() {
    let config = default_config();
    assert!(config.memory.enabled);
    assert_eq!(config.memory.dir, ".d3vx/memory");
    assert_eq!(config.memory.max_entries, 10000);
    assert!(config.memory.auto_learn);
}

#[test]
fn test_default_config_mcp_servers_empty() {
    let config = default_config();
    assert!(config.mcp.servers.is_empty());
}

#[test]
fn test_default_config_budget_enabled() {
    let config = default_config();
    let budget = config.budget.as_ref().unwrap();
    assert!(budget.enabled);
    assert!((budget.per_session - 5.00).abs() < f64::EPSILON);
}

#[test]
fn test_default_config_security_blocklist_empty() {
    let config = default_config();
    assert!(config.security.bash_tool.blocklist.is_empty());
}

#[test]
fn test_default_config_has_provider_configs() {
    let config = default_config();
    let providers = config.providers.configs.as_ref().unwrap();
    assert!(providers.contains_key("anthropic"));
    assert!(providers.contains_key("openai"));
    assert!(providers.contains_key("ollama"));
}

#[test]
fn test_default_config_hooks_default() {
    let config = default_config();
    assert!(config.hooks.session_start.is_none());
    assert!(config.hooks.stop.is_none());
}

#[test]
fn test_default_config_plugins_default() {
    let config = default_config();
    assert!(config.plugins.runtime.is_none());
}

#[test]
fn test_default_config_subagent_cleanup() {
    let config = default_config();
    assert_eq!(config.subagent.cleanup.retention_period_secs, 300);
    assert_eq!(config.subagent.cleanup.cleanup_interval_secs, 60);
}

#[test]
fn test_env_var_map_contains_known_entries() {
    let vars: Vec<_> = ENV_VAR_MAP.iter().map(|(v, _)| *v).collect();
    assert!(vars.contains(&"D3VX_PROVIDER"));
    assert!(vars.contains(&"D3VX_MODEL"));
    assert!(vars.contains(&"D3VX_AUTO_COMMIT"));
    assert!(vars.contains(&"D3VX_MEMORY_ENABLED"));
    assert!(vars.contains(&"D3VX_LSP_ENABLED"));
    assert!(vars.iter().any(|v| v.starts_with("D3VX_UI_")));
}

#[test]
fn test_project_config_dir_path() {
    let result = get_project_config_dir("/home/user/myproject");
    assert_eq!(result, "/home/user/myproject/.d3vx");
}

#[test]
fn test_project_config_path() {
    let result = get_project_config_path("/home/user/myproject");
    assert_eq!(result, "/home/user/myproject/.d3vx/config.yml");
}
