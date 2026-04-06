//! Tests for feature flags config

use super::types_features::FeatureFlagsConfig;

// =========================================================================
// FeatureFlagsConfig tests
// =========================================================================

#[test]
fn test_feature_flags_default() {
    let flags = FeatureFlagsConfig::default();
    assert!(flags.enabled);
    assert!(flags.flags.is_empty());
}

#[test]
fn test_is_enabled_false_by_default() {
    let flags = FeatureFlagsConfig::default();
    assert!(!flags.is_enabled("nonexistent_flag"));
}

#[test]
fn test_is_enabled_granted_from_flags_map() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("experimental_feature".to_string(), true);
    assert!(flags.is_enabled("experimental_feature"));
}

#[test]
fn test_is_enabled_denied_from_flags_map() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("some_feature".to_string(), false);
    assert!(!flags.is_enabled("some_feature"));
}

#[test]
fn test_set_flag() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("foo".to_string(), true);
    assert_eq!(flags.flags.get("foo"), Some(&true));

    flags.set_flag("foo".to_string(), false);
    assert_eq!(flags.flags.get("foo"), Some(&false));
}

#[test]
fn test_feature_flags_set_flag_toggles() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("test".to_string(), true);
    assert!(flags.is_enabled("test"));

    flags.set_flag("test".to_string(), false);
    assert!(!flags.is_enabled("test"));
}

#[test]
fn test_set_flag_does_not_affect_others() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("a".to_string(), true);
    flags.set_flag("b".to_string(), true);

    flags.set_flag("a".to_string(), false);
    assert!(!flags.is_enabled("a"));
    assert!(flags.is_enabled("b"));
}

#[test]
fn test_feature_flags_config_equality() {
    let mut a = FeatureFlagsConfig::default();
    let mut b = FeatureFlagsConfig::default();
    assert_eq!(a, b);

    a.set_flag("x".to_string(), true);
    assert_ne!(a, b);

    b.set_flag("x".to_string(), true);
    assert_eq!(a, b);
}

#[test]
fn test_feature_flags_clone() {
    let mut flags = FeatureFlagsConfig::default();
    flags.set_flag("cloned".to_string(), true);
    let cloned = flags.clone();
    assert_eq!(flags, cloned);
}

#[test]
fn test_feature_flags_default_enabled_true() {
    let flags = FeatureFlagsConfig::default();
    assert!(flags.enabled);
}
