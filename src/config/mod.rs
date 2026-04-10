//! Configuration module for d3vx
//!
//! Handles loading and merging configuration from multiple sources:
//! - Global config: `~/.d3vx/config.yml`
//! - Project config: `.d3vx/config.yml`
//! - Environment variables
//! - CLI flags
//!
//! # API Key Loading
//!
//! API keys are loaded from environment variables, not stored in config files.
//! The resolution order for each provider is:
//! 1. Standard env var (e.g., `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`)
//! 2. Custom env var from provider config (`api_key_env` field)
//! 3. Generic pattern (`{PROVIDER}_API_KEY`)
//!
//! # Example
//!
//! ```ignore
//! use d3vx::config::{load_config, LoadConfigOptions, get_api_key, get_provider_config};
//!
//! // Load config with all sources merged
//! let config = load_config(LoadConfigOptions::default())?;
//!
//! // Get API key for the configured provider
//! let api_key = get_api_key(&config.provider, &config);
//!
//! // Or get everything at once
//! let (model, api_key, base_url) = get_provider_config(&config);
//! ```

pub mod defaults;
pub mod flags;
pub mod keychain;
pub mod loader;
pub mod onboarding;
pub mod security;
pub mod types;

// Re-export main types
pub use crate::providers::SUPPORTED_PROVIDERS;
pub use defaults::{get_global_config_dir, DEFAULT_CONFIG};
pub use flags::{init_feature_flags, is_feature_enabled, set_feature_flag};
pub use keychain::{delete_key, get_key, has_key, store_key};
pub use loader::{get_api_key, get_provider_config, load_config, LoadConfigOptions};
pub use onboarding::{
    check_onboarding_status, format_provider_options, get_doctor_command_hint,
    get_setup_instructions, supported_providers,
};
pub use security::{default_blocklist, BashToolConfig, SecurityConfig, SecurityError};
pub use types::*;
