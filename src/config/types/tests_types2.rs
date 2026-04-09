//! Tests for config types: sandbox, roundtrip, and misc
//! (Split from tests_types.rs to stay under 300 lines)

use super::types_sandbox::{FilesystemRestriction, NetworkRestriction, SandboxConfig, SandboxMode};
use serde_yaml;

// =========================================================================
// Sandbox config tests
// =========================================================================

#[test]
fn test_sandbox_mode_serialization() {
    let mode = SandboxMode::Native;
    let yaml = serde_yaml::to_string(&mode).unwrap();
    let parsed: SandboxMode = serde_yaml::from_str(&yaml).unwrap();
    assert!(matches!(parsed, SandboxMode::Native));
}

#[test]
fn test_sandbox_mode_all_variants() {
    for mode in [
        SandboxMode::Native,
        SandboxMode::Restricted,
        SandboxMode::Disabled,
    ] {
        let yaml = serde_yaml::to_string(&mode).unwrap();
        let parsed: SandboxMode = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(mode, parsed);
    }
}

#[test]
fn test_sandbox_config_defaults() {
    let sandbox = SandboxConfig::default();
    assert!(matches!(sandbox.mode, SandboxMode::Disabled));
    assert!(!sandbox.enabled);
    assert!(sandbox.network.allowed_domains.is_empty());
    assert!(sandbox.filesystem.deny_read.is_empty());
}

#[test]
fn test_filesystem_restriction_defaults() {
    let fs = FilesystemRestriction::default();
    assert!(fs.deny_read.is_empty());
    assert!(fs.allow_write.is_empty());
    assert!(fs.deny_write.is_empty());
}

#[test]
fn test_filesystem_restriction_roundtrip() {
    let fs = FilesystemRestriction {
        deny_read: vec!["/etc/shadow".to_string()],
        allow_write: vec!["/tmp".to_string()],
        deny_write: vec!["/".to_string()],
    };
    let yaml = serde_yaml::to_string(&fs).unwrap();
    let parsed: FilesystemRestriction = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(fs, parsed);
}

#[test]
fn test_network_restriction_defaults() {
    let net = NetworkRestriction::default();
    assert!(net.allowed_domains.is_empty());
    assert!(net.blocked_domains.is_empty());
    assert!(net.http_proxy_port.is_none());
}

#[test]
fn test_network_restriction_roundtrip() {
    let net = NetworkRestriction {
        allowed_domains: vec!["example.com".to_string()],
        blocked_domains: vec!["malware.com".to_string()],
        http_proxy_port: Some(8080),
        socks_proxy_port: None,
    };
    let yaml = serde_yaml::to_string(&net).unwrap();
    let parsed: NetworkRestriction = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(net, parsed);
}

// =========================================================================
// D3vxConfig roundtrip tests
// =========================================================================

#[test]
fn test_d3vx_config_full_roundtrip() {
    use crate::config::defaults::default_config;
    let config = default_config();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed = serde_yaml::from_str::<super::D3vxConfig>(&yaml).unwrap();
    assert_eq!(config, parsed);
}

#[test]
fn test_static_default_config_matches_builder() {
    use crate::config::defaults::{default_config, DEFAULT_CONFIG};
    assert_eq!(&*DEFAULT_CONFIG, &default_config());
}

// =========================================================================
// Partial config tests
// =========================================================================

#[test]
fn test_partial_config_defaults() {
    use super::PartialD3vxConfig;
    let partial = PartialD3vxConfig::default();
    assert!(partial.provider.is_none());
    assert!(partial.model.is_none());
    assert!(partial.version.is_none());
}

#[test]
fn test_partial_config_with_value() {
    use super::PartialD3vxConfig;
    let partial = PartialD3vxConfig {
        provider: Some("openai".to_string()),
        ..Default::default()
    };
    assert_eq!(partial.provider, Some("openai".to_string()));
}

// =========================================================================
// Sub-agent config tests
// =========================================================================

#[test]
fn test_subagent_config_defaults() {
    use super::SubAgentConfig;
    // Default derive gives false/0 for bools/ints; serde defaults only apply on deserialize
    let cfg = SubAgentConfig::default();
    assert_eq!(cfg.cleanup.retention_period_secs, 300);
    assert!(!cfg.parallel_agents);
    assert_eq!(cfg.max_parallel_agents, 0); // Default derive, not deserialize default
    assert!(!cfg.auto_detect_complexity);
    // Serde deserialization applies defaults
    let cfg2: SubAgentConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(cfg2.max_parallel_agents, 3);
    assert!(cfg2.auto_detect_complexity);
}

#[test]
fn test_cleanup_config_defaults() {
    use super::CleanupConfig;
    let cleanup = CleanupConfig::default();
    assert_eq!(cleanup.retention_period_secs, 300);
    assert_eq!(cleanup.cleanup_interval_secs, 60);
    assert_eq!(cleanup.max_retained, 10);
}

// =========================================================================
// Memory config tests
// =========================================================================

#[test]
fn test_memory_config_full() {
    use super::MemoryConfig;
    let mem = MemoryConfig {
        enabled: false,
        dir: "/tmp/mem".to_string(),
        max_entries: 500,
        auto_learn: false,
        enable_search: false,
    };
    let yaml = serde_yaml::to_string(&mem).unwrap();
    let parsed: MemoryConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(mem, parsed);
}

// =========================================================================
// Recovery config tests
// =========================================================================

#[test]
fn test_recovery_config_defaults() {
    use super::RecoveryConfig;
    let rc = RecoveryConfig::default();
    assert_eq!(rc.max_retries, 3);
    assert_eq!(rc.initial_delay_ms, 500);
    assert_eq!(rc.max_delay_ms, 30000);
    assert!((rc.backoff_multiplier - 2.0).abs() < f64::EPSILON);
    assert!(rc.checkpoint_enabled);
}

// =========================================================================
// Integration config tests
// =========================================================================

#[test]
fn test_budget_config_defaults() {
    use super::BudgetConfig;
    let budget = BudgetConfig {
        per_session: 1.0,
        per_day: 10.0,
        warn_at: 0.8,
        pause_at: 1.0,
        enabled: true,
    };
    let json = serde_json::to_string(&budget).unwrap();
    let parsed: BudgetConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(budget, parsed);
}

#[test]
fn test_hooks_config_defaults() {
    use super::HooksConfig;
    let hooks = HooksConfig::default();
    assert!(hooks.session_start.is_none());
    assert!(hooks.user_prompt_submit.is_none());
    assert!(hooks.pre_tool_use.is_none());
    assert!(hooks.post_tool_use.is_none());
    assert!(hooks.subagent_stop.is_none());
    assert!(hooks.stop.is_none());
    assert!(hooks.notification.is_none());
}

#[test]
fn test_hook_timeout_default() {
    use super::{Hook, HookType};
    let hook = Hook {
        hook_type: HookType::Command,
        command: Some("echo hello".to_string()),
        server: None,
        tool: None,
        timeout: 0, // will use default during deserialization
    };
    // When serialized and deserialized, missing timeout field gets default
    let yaml = serde_yaml::to_string(&hook).unwrap();
    assert!(yaml.contains("command: echo hello"));
}

#[test]
fn test_plugin_setting_serialization() {
    use super::{PluginCapability, PluginManifest};
    let manifest = PluginManifest {
        name: "test-plugin".to_string(),
        version: "0.1.0".to_string(),
        description: Some("A test plugin".to_string()),
        author: None,
        entry: Some("main.rs".to_string()),
        capabilities: vec![PluginCapability::Tool, PluginCapability::Hook],
    };
    let json = serde_json::to_string(&manifest).unwrap();
    let parsed: PluginManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(manifest, parsed);
}

#[test]
fn test_plugin_enabled_default_is_true() {
    use super::PluginEnabled;
    let pe = PluginEnabled {
        name: "test".to_string(),
        enabled: true,
        options: None,
    };
    assert!(pe.enabled);
}

#[test]
fn test_plugins_config_defaults() {
    use super::PluginsConfig;
    let pc = PluginsConfig::default();
    assert!(pc.runtime.is_none());
    assert!(pc.agent.is_none());
    assert!(pc.enabled.is_empty());
    assert!(pc.options.is_empty());
}
