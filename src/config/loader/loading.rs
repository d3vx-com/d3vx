//! Main configuration loading logic

use super::super::defaults::{get_global_config_path, get_project_config_path, DEFAULT_CONFIG};
use super::super::security::SecurityConfig;
use super::super::types::D3vxConfig;
use super::merging::{deep_merge, load_env_overrides};
use super::LoadConfigOptions;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use tracing::debug;

/// Load and merge configuration from all sources
///
/// Precedence (highest to lowest):
/// 1. CLI flags (cli_overrides)
/// 2. Environment variables
/// 3. Project config (.d3vx/config.yml)
/// 4. Global config (~/.d3vx/config.yml)
/// 5. Defaults
pub fn load_config(options: LoadConfigOptions) -> Result<D3vxConfig> {
    let project_root = options
        .project_root
        .clone()
        .or_else(find_project_root)
        .unwrap_or_else(|| {
            env::current_dir()
                .context("Failed to get current directory for config loading")
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    debug!(
        "Loading configuration (project_root={}, skip_global={}, skip_project={}, skip_env={})",
        project_root, options.skip_global, options.skip_project, options.skip_env
    );

    // Start with defaults
    let mut config_map: serde_yaml::Value =
        serde_yaml::to_value(&*DEFAULT_CONFIG).context("Failed to serialize defaults")?;

    // Merge global config
    if !options.skip_global {
        let global_path = get_global_config_path();
        if let Some(global_config) = load_config_file(&global_path)? {
            config_map = deep_merge(&config_map, &global_config);
            debug!("Merged global config from {}", global_path);
        }
    }

    // Merge project config
    if !options.skip_project {
        let project_path = get_project_config_path(&project_root);
        if let Some(project_config) = load_config_file(&project_path)? {
            config_map = deep_merge(&config_map, &project_config);
            debug!("Merged project config from {}", project_path);
        }
    }

    // Load security.toml and merge into config
    if !options.skip_security {
        let security_path = SecurityConfig::get_default_path(&project_root);
        match SecurityConfig::load_from_file(&security_path) {
            Ok(security_config) => {
                let security_yaml = serde_yaml::to_value(&security_config)
                    .context("Failed to serialize security config")?;
                config_map = deep_merge(&config_map, &security_yaml);
                debug!("Merged security config from {}", security_path);
            }
            Err(e) => {
                debug!("Could not load security config: {}", e);
            }
        }
    }

    // Apply environment variable overrides
    if !options.skip_env {
        let env_overrides = load_env_overrides();
        if !env_overrides.is_null() && !env_overrides.as_mapping().map_or(true, |m| m.is_empty()) {
            config_map = deep_merge(&config_map, &env_overrides);
            debug!("Applied environment overrides");
        }
    }

    // Apply CLI overrides
    if !options.cli_overrides.is_empty() {
        let cli_map = serde_yaml::to_value(&options.cli_overrides)
            .context("Failed to serialize CLI overrides")?;
        config_map = deep_merge(&config_map, &cli_map);
        debug!("Applied CLI overrides");
    }

    // Deserialize back to D3vxConfig
    let config: D3vxConfig =
        serde_yaml::from_value(config_map).context("Failed to deserialize merged config")?;

    debug!(
        "Configuration loaded (provider={}, model={})",
        config.provider, config.model
    );

    Ok(config)
}

/// Load and parse a YAML config file
pub fn load_config_file(path: &str) -> Result<Option<serde_yaml::Value>> {
    let path = Path::new(path);

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let value: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    debug!("Loaded config from {}", path.display());
    Ok(Some(value))
}

/// Find the project root by walking up from the current directory
/// searching for a .d3vx folder.
pub fn find_project_root() -> Option<String> {
    let mut current_dir = env::current_dir().ok()?;

    loop {
        if current_dir.join(".d3vx").is_dir() {
            return Some(current_dir.to_string_lossy().to_string());
        }

        if !current_dir.pop() {
            break;
        }
    }

    None
}

/// Save a partial configuration to the project config file
pub fn save_project_config_part(project_root: &str, part: serde_yaml::Value) -> Result<()> {
    let path = get_project_config_path(project_root);
    save_config_file_part(&path, part)
}

/// Save a partial configuration to the global config file
pub fn save_global_config_part(part: serde_yaml::Value) -> Result<()> {
    let path = get_global_config_path();
    save_config_file_part(&path, part)
}

/// Merge a partial YAML value into an existing YAML file
fn save_config_file_part(path: &str, part: serde_yaml::Value) -> Result<()> {
    let mut current_val = if Path::new(path).exists() {
        load_config_file(path)?.unwrap_or(serde_yaml::Value::Mapping(Default::default()))
    } else {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        serde_yaml::Value::Mapping(Default::default())
    };

    current_val = deep_merge(&current_val, &part);

    let serialized =
        serde_yaml::to_string(&current_val).context("Failed to serialize merged config")?;

    fs::write(path, serialized)
        .with_context(|| format!("Failed to write config file: {}", path))?;

    debug!("Saved partial config to {}", path);
    Ok(())
}
