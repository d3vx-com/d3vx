//! Tests for config type serialization, defaults, and validation

use serde_yaml;
use std::collections::HashMap;

use super::types_mcp::McpServer;
use super::types_notifications::TelegramConfig;
use super::{NotificationsConfig, PreCommitConfig, UiMode};

// =========================================================================
// UiMode tests
// =========================================================================

#[test]
fn test_ui_mode_default_is_chat() {
    let mode = UiMode::default();
    assert!(matches!(mode, UiMode::Chat));
}

#[test]
fn test_ui_mode_serialization_roundtrip() {
    let modes = vec![
        UiMode::Chat,
        UiMode::Kanban,
        UiMode::List,
        UiMode::Suggestion,
    ];
    for mode in &modes {
        let yaml = serde_yaml::to_string(mode).unwrap();
        let parsed: UiMode = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(*mode, parsed);
    }
}

#[test]
fn test_ui_mode_yaml_format() {
    // UiMode uses #[serde(rename_all = "lowercase")]
    let yaml = serde_yaml::to_string(&UiMode::Kanban).unwrap();
    assert!(yaml.contains("kanban"));
}

// =========================================================================
// Pipeline config tests
// =========================================================================

#[test]
fn test_pipeline_phase_defaults() {
    use super::PipelinePhase;
    let phase = PipelinePhase {
        enabled: false,
        max_retries: 0,
        model: None,
        timeout_ms: None,
    };
    let yaml = serde_yaml::to_string(&phase).unwrap();
    let parsed: PipelinePhase = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(phase, parsed);
}

#[test]
fn test_pipeline_phase_model_serialization() {
    use super::PipelinePhase;
    let phase = PipelinePhase {
        enabled: true,
        max_retries: 2,
        model: Some("gpt-4".to_string()),
        timeout_ms: None,
    };
    let yaml = serde_yaml::to_string(&phase).unwrap();
    let parsed: PipelinePhase = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(phase, parsed);
    assert!(yaml.contains("gpt-4"));
}

#[test]
fn test_pipeline_config_skip_serializing_none() {
    let phases = super::PipelinePhases {
        research: super::PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        plan: super::PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        implement: super::PipelinePhase {
            enabled: true,
            max_retries: 3,
            model: None,
            timeout_ms: None,
        },
        review: super::PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
        docs: super::PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
        learn: super::PipelinePhase {
            enabled: true,
            max_retries: 1,
            model: None,
            timeout_ms: None,
        },
    };
    let config = super::PipelineConfig {
        phases,
        stop_on_failure: true,
        checkpoint: true,
        budget: None,
        timeouts: None,
        max_concurrent_agents: 3,
    };
    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(!yaml.contains("budget"));
    assert!(!yaml.contains("timeouts"));
}

#[test]
fn test_model_routing_serialization() {
    use super::ModelRouting;
    let mr = ModelRouting {
        enabled: true,
        cheap_model: Some("gpt-3.5-turbo".to_string()),
        standard_model: None,
        premium_model: None,
        complexity_routing: false,
    };
    let yaml = serde_yaml::to_string(&mr).unwrap();
    assert!(yaml.contains("cheap_model"));
}

// =========================================================================
// Git config tests
// =========================================================================

#[test]
fn test_git_config_defaults() {
    use super::GitConfig;
    // Construct via Default; PreCommitConfig implements Default
    let pre_commit = PreCommitConfig::default();
    let git = GitConfig {
        auto_commit: true,
        auto_push: false,
        worktree_dir: ".d3vx-worktrees".to_string(),
        commit_prefix: "feat".to_string(),
        ai_commit_messages: true,
        commit_message_max_tokens: 100,
        main_branch: "main".to_string(),
        sign_commits: false,
        pre_commit_hooks: pre_commit.clone(),
    };
    assert!(matches!(
        git.pre_commit_hooks,
        PreCommitConfig { format: true, .. }
    ));

    // Roundtrip
    let yaml = serde_yaml::to_string(&git).unwrap();
    let parsed: GitConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(git, parsed);
}

#[test]
fn test_pre_commit_config_all_true_by_default() {
    let config = PreCommitConfig::default();
    assert!(config.format);
    assert!(config.clippy);
    assert!(config.test);
    assert!(config.security);
    assert!(config.skip_if_wip);
    assert_eq!(config.timeout_seconds, 60);
}

#[test]
fn test_pre_commit_config_serialization() {
    let config = PreCommitConfig::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: PreCommitConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config, parsed);
}

// =========================================================================
// Provider config tests
// =========================================================================

#[test]
fn test_provider_config_serialization() {
    use super::ProviderConfig;
    let pc = ProviderConfig {
        api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
        default_model: "claude-sonnet-4-20250514".to_string(),
        base_url: None,
        research_model: Some("claude-haiku-4-20250514".to_string()),
        cheap_model: None,
        timeout_ms: Some(60000),
        max_retries: Some(3),
    };
    let yaml = serde_yaml::to_string(&pc).unwrap();
    let parsed: ProviderConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(pc, parsed);
}

#[test]
fn test_providers_config_defaults() {
    use super::ProvidersConfig;
    let pc = ProvidersConfig::default();
    assert!(pc.fallback_chain.is_none());
    assert!(pc.configs.is_none());
}

#[test]
fn test_providers_config_with_configs() {
    use super::{ProviderConfig, ProvidersConfig};
    let mut configs = HashMap::new();
    configs.insert(
        "openai".to_string(),
        ProviderConfig {
            api_key_env: None,
            default_model: "gpt-4o".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            research_model: None,
            cheap_model: Some("gpt-4o-mini".to_string()),
            timeout_ms: None,
            max_retries: None,
        },
    );
    let pc = ProvidersConfig {
        fallback_chain: None,
        configs: Some(configs),
    };
    assert!(pc.configs.as_ref().unwrap().contains_key("openai"));
}

// =========================================================================
// MCP config tests
// =========================================================================

#[test]
fn test_mcp_server_serialization() {
    let server = McpServer {
        command: "npx".to_string(),
        args: vec!["@modelcontextprotocol/server-filesystem".to_string()],
        env: None,
        cwd: None,
        timeout_ms: Some(5000),
    };
    let yaml = serde_yaml::to_string(&server).unwrap();
    let parsed: McpServer = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(server, parsed);
}

#[test]
fn test_mcp_config_defaults() {
    use super::McpConfig;
    let config = McpConfig::default();
    assert!(config.servers.is_empty());
}

#[test]
fn test_mcp_server_with_env() {
    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());
    let server = McpServer {
        command: "echo".to_string(),
        args: vec![],
        env: Some(env.clone()),
        cwd: Some("/tmp".to_string()),
        timeout_ms: None,
    };
    assert_eq!(server.env.unwrap().get("FOO"), Some(&"bar".to_string()));
}

// =========================================================================
// Notification config tests
// =========================================================================

#[test]
fn test_telegram_config_serialization() {
    let tg = NotificationsConfig {
        desktop: true,
        telegram: Some(TelegramConfig {
            bot_token: "my-bot-token".to_string(),
            chat_id: "123456".to_string(),
        }),
        on_task_done: true,
        on_task_failed: true,
        on_mergeable: true,
    };
    let json = serde_json::to_string(&tg).unwrap();
    let parsed: NotificationsConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(tg, parsed);
}

#[test]
fn test_telegram_config_fields() {
    let tg = TelegramConfig {
        bot_token: "token".to_string(),
        chat_id: "chat".to_string(),
    };
    assert_eq!(tg.bot_token, "token");
    assert_eq!(tg.chat_id, "chat");
}
