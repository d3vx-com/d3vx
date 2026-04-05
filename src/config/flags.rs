//! Feature Flag Store
//!
//! Global singleton for runtime feature flag access.

use once_cell::sync::Lazy;
use std::sync::RwLock;
use tracing::debug;

use super::types::types_features::FeatureFlagsConfig;

static FEATURE_FLAGS: Lazy<RwLock<FeatureFlagsConfig>> =
    Lazy::new(|| RwLock::new(FeatureFlagsConfig::default()));

/// Check if a feature flag is enabled.
pub fn is_feature_enabled(flag: &str) -> bool {
    FEATURE_FLAGS.read().unwrap().is_enabled(flag)
}

/// Set a feature flag at runtime.
pub fn set_feature_flag(flag: &str, enabled: bool) {
    debug!(flag = flag, enabled = enabled, "Setting feature flag");
    let mut flags = FEATURE_FLAGS.write().unwrap();
    flags.set_flag(flag.to_string(), enabled);
}

/// Initialize feature flags from config.
pub fn init_feature_flags(config: &FeatureFlagsConfig) {
    let mut flags = FEATURE_FLAGS.write().unwrap();
    *flags = config.clone();
    debug!("Initialized feature flags from config");
}

/// Check feature with default from definitions.
pub fn feature(flag: &str) -> bool {
    if is_feature_enabled(flag) {
        return true;
    }
    false
}
