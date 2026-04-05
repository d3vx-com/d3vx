//! Configuration merging and environment variable overrides

use super::super::defaults;
use std::collections::HashMap;
use std::env;

/// Deep merge two YAML values
///
/// - For objects/mappings: recursively merge, source values override target
/// - For arrays: source replaces target
/// - For primitives: source replaces target
pub fn deep_merge(target: &serde_yaml::Value, source: &serde_yaml::Value) -> serde_yaml::Value {
    match (target, source) {
        (serde_yaml::Value::Mapping(target_map), serde_yaml::Value::Mapping(source_map)) => {
            let mut result = target_map.clone();
            for (key, source_value) in source_map {
                if let Some(target_value) = result.get(key) {
                    result.insert(key.clone(), deep_merge(target_value, source_value));
                } else {
                    result.insert(key.clone(), source_value.clone());
                }
            }
            serde_yaml::Value::Mapping(result)
        }
        _ => source.clone(),
    }
}

/// Load config overrides from environment variables
pub fn load_env_overrides() -> serde_yaml::Value {
    let mut overrides: HashMap<String, String> = HashMap::new();

    for (env_var, path) in defaults::ENV_VAR_MAP {
        if let Ok(val) = env::var(env_var) {
            overrides.insert(path.to_string(), val);
        }
    }

    // Special case for provider-specific base URLs
    if let Ok(val) = env::var("D3VX_ANTHROPIC_BASE_URL") {
        overrides.insert("providers.configs.anthropic.base_url".to_string(), val);
    }

    let mut result = serde_yaml::Mapping::new();
    for (path, value) in overrides {
        set_nested_path(&mut result, &path, parse_env_value(&value));
    }

    serde_yaml::Value::Mapping(result)
}

/// Parse an environment variable value to YAML value
pub fn parse_env_value(value: &str) -> serde_yaml::Value {
    match value.to_lowercase().as_str() {
        "true" => serde_yaml::Value::Bool(true),
        "false" => serde_yaml::Value::Bool(false),
        s if s.parse::<i64>().is_ok() => {
            serde_yaml::Value::Number(s.parse::<i64>().unwrap().into())
        }
        s if s.parse::<f64>().is_ok() => {
            serde_yaml::Value::Number(s.parse::<f64>().unwrap().into())
        }
        s => serde_yaml::Value::String(s.to_string()),
    }
}

/// Set a value at a nested path in a YAML mapping
pub fn set_nested_path(mapping: &mut serde_yaml::Mapping, path: &str, value: serde_yaml::Value) {
    let parts: Vec<&str> = path.split('.').collect();
    set_nested_path_recursive(mapping, &parts, 0, value);
}

fn set_nested_path_recursive(
    mapping: &mut serde_yaml::Mapping,
    parts: &[&str],
    index: usize,
    value: serde_yaml::Value,
) {
    if index >= parts.len() {
        return;
    }

    let key = serde_yaml::Value::String(parts[index].to_string());

    if index == parts.len() - 1 {
        mapping.insert(key, value);
    } else {
        let next_mapping = mapping
            .entry(key)
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

        if let serde_yaml::Value::Mapping(ref mut inner) = next_mapping {
            set_nested_path_recursive(inner, parts, index + 1, value);
        }
    }
}
