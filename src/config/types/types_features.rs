//! Feature Flags Configuration
//!
//! Runtime feature flags for controlling d3vx behavior.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature flags configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct FeatureFlagsConfig {
    /// Whether feature flags are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Feature flag overrides (flag_name -> enabled)
    #[serde(default)]
    pub flags: HashMap<String, bool>,
}

fn default_true() -> bool {
    true
}

impl Default for FeatureFlagsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            flags: HashMap::new(),
        }
    }
}

impl FeatureFlagsConfig {
    /// Check if a feature flag is enabled.
    /// Resolution order: explicit flags > env vars > defaults
    pub fn is_enabled(&self, flag: &str) -> bool {
        // Check explicit overrides first
        if let Some(enabled) = self.flags.get(flag) {
            return *enabled;
        }
        // Check environment variable: D3VX_FEATURE_<FLAG>=true/false
        let env_key = format!("D3VX_FEATURE_{}", flag.to_uppercase());
        if let Ok(val) = std::env::var(&env_key) {
            return val == "true";
        }
        // Default: disabled
        false
    }

    /// Set a feature flag at runtime.
    pub fn set_flag(&mut self, flag: String, enabled: bool) {
        self.flags.insert(flag, enabled);
    }
}

/// Common feature flag names
pub mod feature_flags {
    pub const READ_BEFORE_WRITE: &str = "read_before_write";
    pub const AUTO_COMPACT: &str = "auto_compact";
    pub const SANDBOX_ENABLED: &str = "sandbox_enabled";
    pub const WEB_SEARCH_ENABLED: &str = "web_search_enabled";
    pub const BACKGROUND_TASKS: &str = "background_tasks";
    pub const CRON_SCHEDULING: &str = "cron_scheduling";
}
