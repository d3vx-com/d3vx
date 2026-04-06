//! Configuration loader with layered precedence
//!
//! Loads configuration from multiple sources with proper precedence:
//! 1. CLI flags (highest)
//! 2. Environment variables
//! 3. Security config (.d3vx/security.toml)
//! 4. Project config (.d3vx/config.yml)
//! 5. Global config (~/.d3vx/config.yml)
//! 6. Defaults (lowest)

mod api_keys;
pub mod loading;
mod merging;

#[cfg(test)]
mod merging_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

pub use api_keys::{get_api_key, get_provider_config};
pub use loading::{find_project_root, load_config, load_config_file};
pub use merging::{deep_merge, load_env_overrides, parse_env_value, set_nested_path};

/// Options for loading configuration
#[derive(Debug, Clone, Default)]
pub struct LoadConfigOptions {
    /// Project root directory
    pub project_root: Option<String>,
    /// CLI flag overrides as key-value pairs
    pub cli_overrides: HashMap<String, serde_json::Value>,
    /// Skip loading global config
    pub skip_global: bool,
    /// Skip loading project config
    pub skip_project: bool,
    /// Skip environment variable overrides
    pub skip_env: bool,
    /// Skip loading security.toml
    pub skip_security: bool,
}
