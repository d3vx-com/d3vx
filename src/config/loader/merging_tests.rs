//! Tests for config merging and environment overrides

use std::sync::Mutex;

// Reentrant lock to serialize tests that manipulate env vars
static ENV_LOCK: Mutex<()> = Mutex::new(());

use super::merging::{deep_merge, load_env_overrides, parse_env_value, set_nested_path};

// =========================================================================
// deep_merge tests
// =========================================================================

#[test]
fn test_merge_empty_target() {
    let target = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    let source = serde_yaml::from_str::<serde_yaml::Value>("a: 1").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["a"], 1);
}

#[test]
fn test_merge_empty_source() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("a: 1\nb: 2").unwrap();
    let source = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    let result = deep_merge(&target, &source);
    assert_eq!(result["a"], 1);
    assert_eq!(result["b"], 2);
}

#[test]
fn test_merge_primitive_overrides() {
    let target =
        serde_yaml::from_str::<serde_yaml::Value>("provider: openai\nmodel: gpt-4").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("provider: anthropic").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["provider"], "anthropic");
    assert_eq!(result["model"], "gpt-4"); // preserved from target
}

#[test]
fn test_merge_deeply_nested() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("a:\n  b:\n    c:\n      d: 1").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("a:\n  b:\n    c:\n      e: 2").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["a"]["b"]["c"]["d"], 1);
    assert_eq!(result["a"]["b"]["c"]["e"], 2);
}

#[test]
fn test_merge_replaces_non_mapping_with_mapping() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("key: scalar").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("key:\n  a: 1").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["key"]["a"], 1);
}

#[test]
fn test_merge_replaces_mapping_with_non_mapping() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("key:\n  a: 1").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("key: scalar").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["key"], "scalar");
}

#[test]
fn test_merge_arrays_replace() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("items:\n  - a\n  - b").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("items:\n  - c").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["items"].as_sequence().unwrap().len(), 1);
    assert_eq!(result["items"][0], "c");
}

#[test]
fn test_merge_bool_values() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("enabled: false").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("enabled: true").unwrap();
    let result = deep_merge(&target, &source);
    assert_eq!(result["enabled"].as_bool(), Some(true));
}

// =========================================================================
// parse_env_value tests
// =========================================================================

#[test]
fn test_parse_bool_true() {
    assert_eq!(parse_env_value("true"), serde_yaml::Value::Bool(true));
    assert_eq!(parse_env_value("TRUE"), serde_yaml::Value::Bool(true));
    assert_eq!(parse_env_value("True"), serde_yaml::Value::Bool(true));
}

#[test]
fn test_parse_bool_false() {
    assert_eq!(parse_env_value("false"), serde_yaml::Value::Bool(false));
    assert_eq!(parse_env_value("FALSE"), serde_yaml::Value::Bool(false));
}

#[test]
fn test_parse_int() {
    assert_eq!(
        parse_env_value("42"),
        serde_yaml::Value::Number(42i64.into())
    );
    assert_eq!(
        parse_env_value("-1"),
        serde_yaml::Value::Number((-1i64).into())
    );
    assert_eq!(parse_env_value("0"), serde_yaml::Value::Number(0i64.into()));
}

#[test]
fn test_parse_float() {
    assert_eq!(
        parse_env_value("3.14"),
        serde_yaml::Value::Number(3.14f64.into())
    );
}

#[test]
fn test_parse_string() {
    let result = parse_env_value("hello world");
    assert_eq!(result.as_str().unwrap(), "hello world");

    // "true" and "false" are case-insensitive; other text stays as string (lowercased)
    let result = parse_env_value("Hello");
    assert_eq!(result.as_str().unwrap(), "hello");
}

// =========================================================================
// set_nested_path tests
// =========================================================================

#[test]
fn test_set_nested_path_single_level() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "key", serde_yaml::Value::String("val".into()));
    assert_eq!(mapping["key"], "val");
}

#[test]
fn test_set_nested_path_two_levels() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "a.b", serde_yaml::Value::Number(7i64.into()));
    assert_eq!(mapping["a"]["b"], 7);
}

#[test]
fn test_set_nested_path_deep() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "x.y.z.w", serde_yaml::Value::Bool(true));
    assert!(mapping["x"]["y"]["z"]["w"].as_bool().unwrap());
}

#[test]
fn test_set_nested_path_overwrites_existing() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "a.b", serde_yaml::Value::Number(1i64.into()));
    set_nested_path(&mut mapping, "a.b", serde_yaml::Value::Number(2i64.into()));
    assert_eq!(mapping["a"]["b"], 2);
}

#[test]
fn test_set_nested_path_empty_path() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "", serde_yaml::Value::Number(99i64.into()));
    // Empty string results in one empty key; value should not be set because
    // the recursive function returns when path parts are empty or the loop doesn't produce results.
    // Actually, split on "" gives [""], then it should set the key "" to value 99.
    assert_eq!(mapping.len(), 1);
}

// =========================================================================
// load_env_overrides tests (require env manipulation)
// =========================================================================

#[test]
fn test_load_env_overrides_empty_by_default() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Clear any test pollution
    for (env_var, _) in crate::config::defaults::ENV_VAR_MAP {
        std::env::remove_var(env_var);
    }
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    let result = load_env_overrides();
    let mapping = result.as_mapping().unwrap();
    assert!(mapping.is_empty());
}

#[test]
fn test_load_env_overrides_single_var() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Clean slate
    for (env_var, _) in crate::config::defaults::ENV_VAR_MAP {
        std::env::remove_var(env_var);
    }
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    std::env::set_var("D3VX_PROVIDER", "openai");
    let result = load_env_overrides();
    std::env::remove_var("D3VX_PROVIDER");

    assert_eq!(result["provider"], "openai");
}

#[test]
fn test_load_env_overrides_bool_parsing() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (env_var, _) in crate::config::defaults::ENV_VAR_MAP {
        std::env::remove_var(env_var);
    }
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    std::env::set_var("D3VX_AUTO_COMMIT", "true");
    let result = load_env_overrides();
    std::env::remove_var("D3VX_AUTO_COMMIT");

    assert!(result["git"]
        .as_mapping()
        .and_then(|m| m.get(&serde_yaml::Value::String("auto_commit".into())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false));
}

#[test]
fn test_load_env_overrides_int_parsing() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (env_var, _) in crate::config::defaults::ENV_VAR_MAP {
        std::env::remove_var(env_var);
    }
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    std::env::set_var("D3VX_MAX_ENTRIES", "500");
    let result = load_env_overrides();
    std::env::remove_var("D3VX_MAX_ENTRIES");

    assert_eq!(result["memory"]["max_entries"].as_i64(), Some(500));
}

#[test]
fn test_load_env_overrides_anthropic_base_url() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (env_var, _) in crate::config::defaults::ENV_VAR_MAP {
        std::env::remove_var(env_var);
    }
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    std::env::set_var("D3VX_ANTHROPIC_BASE_URL", "http://localhost:8080");
    let result = load_env_overrides();
    std::env::remove_var("D3VX_ANTHROPIC_BASE_URL");

    assert_eq!(
        result["providers"]["configs"]["anthropic"]["base_url"]
            .as_str()
            .unwrap(),
        "http://localhost:8080"
    );
}
