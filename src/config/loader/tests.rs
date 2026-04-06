//! Tests for configuration loading

use super::api_keys::{get_api_key, get_provider_config};
use super::loading::load_config;
use super::merging::{deep_merge, parse_env_value, set_nested_path};
use super::LoadConfigOptions;

#[test]
fn test_deep_merge_simple() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("a: 1\nb: 2").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("b: 3\nc: 4").unwrap();

    let result = deep_merge(&target, &source);

    assert_eq!(result["a"], 1);
    assert_eq!(result["b"], 3);
    assert_eq!(result["c"], 4);
}

#[test]
fn test_deep_merge_nested() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("nested:\n  a: 1\n  b: 2").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("nested:\n  b: 3\n  c: 4").unwrap();

    let result = deep_merge(&target, &source);

    assert_eq!(result["nested"]["a"], 1);
    assert_eq!(result["nested"]["b"], 3);
    assert_eq!(result["nested"]["c"], 4);
}

#[test]
fn test_deep_merge_arrays_replace() {
    let target = serde_yaml::from_str::<serde_yaml::Value>("items:\n  - a\n  - b").unwrap();
    let source = serde_yaml::from_str::<serde_yaml::Value>("items:\n  - c\n  - d").unwrap();

    let result = deep_merge(&target, &source);

    assert_eq!(result["items"].as_sequence().unwrap().len(), 2);
    assert_eq!(result["items"][0], "c");
    assert_eq!(result["items"][1], "d");
}

#[test]
fn test_set_nested_path() {
    let mut mapping = serde_yaml::Mapping::new();
    set_nested_path(&mut mapping, "a.b.c", serde_yaml::Value::Number(42.into()));

    assert_eq!(mapping["a"]["b"]["c"], 42);
}

#[test]
fn test_parse_env_value() {
    assert_eq!(parse_env_value("true"), serde_yaml::Value::Bool(true));
    assert_eq!(parse_env_value("false"), serde_yaml::Value::Bool(false));
    assert_eq!(
        parse_env_value("42"),
        serde_yaml::Value::Number(42i64.into())
    );
    assert_eq!(
        parse_env_value("3.14"),
        serde_yaml::Value::Number(3.14f64.into())
    );
    assert_eq!(
        parse_env_value("hello"),
        serde_yaml::Value::String("hello".to_string())
    );
}

#[test]
fn test_load_config_defaults() {
    let options = LoadConfigOptions {
        skip_global: true,
        skip_project: true,
        skip_env: true,
        ..Default::default()
    };
    let config = load_config(options).unwrap();

    assert_eq!(config.version, 1);
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_get_api_key_standard_env() {
    let options = LoadConfigOptions {
        skip_global: true,
        skip_project: true,
        skip_env: true,
        ..Default::default()
    };
    let config = load_config(options).unwrap();

    let _ = get_api_key("anthropic", &config);
    let _ = get_api_key("openai", &config);
    let _ = get_api_key("gemini", &config);
    let _ = get_api_key("ollama", &config);
}

#[test]
fn test_get_provider_config_returns_tuple() {
    let options = LoadConfigOptions {
        skip_global: true,
        skip_project: true,
        skip_env: true,
        ..Default::default()
    };
    let config = load_config(options).unwrap();

    let base_url_backup = std::env::var("ANTHROPIC_BASE_URL").ok();
    std::env::remove_var("ANTHROPIC_BASE_URL");

    let (model, _api_key, base_url) = get_provider_config(&config);

    if let Some(val) = base_url_backup {
        std::env::set_var("ANTHROPIC_BASE_URL", val);
    }

    assert_eq!(model, config.model);
    assert!(base_url.is_none());
}

#[test]
fn test_get_api_key_unknown_provider() {
    let config = load_config(LoadConfigOptions::default()).unwrap();
    let result = get_api_key("unknown_provider_xyz", &config);
    assert!(result.is_none());
}
