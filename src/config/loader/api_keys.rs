//! API key resolution and provider configuration lookup

use super::super::types::D3vxConfig;
use crate::config::keychain;
use crate::providers::SUPPORTED_PROVIDERS;
use std::env;
use tracing::{debug, warn};

/// Get the API key for a specific provider.
///
/// Resolution order:
/// 1. Registry-defined environment variable (canonical source)
/// 2. Custom env var from provider config (api_key_env field)
/// 3. Generic pattern: PROVIDERNAME_API_KEY
/// 4. OS keychain (stored during `d3vx setup`)
pub fn get_api_key(provider_name: &str, config: &D3vxConfig) -> Option<String> {
    let mut env_vars_to_check: Vec<String> = Vec::new();

    if let Some(registry_key) = SUPPORTED_PROVIDERS.api_key_env(provider_name) {
        env_vars_to_check.push(registry_key.to_string());
    }

    if let Some(ref configs) = config.providers.configs {
        if let Some(provider_config) = configs.get(provider_name) {
            if let Some(ref custom_env) = provider_config.api_key_env {
                if !env_vars_to_check.contains(custom_env) {
                    env_vars_to_check.push(custom_env.clone());
                }
            }
        }
    }

    let generic_key = format!("{}_API_KEY", provider_name.to_uppercase());
    if !env_vars_to_check.contains(&generic_key) {
        env_vars_to_check.push(generic_key);
    }

    // 1. Check environment variables first
    for env_var in env_vars_to_check {
        if let Ok(key) = env::var(&env_var) {
            if !key.is_empty() {
                debug!("Found API key for {} from {}", provider_name, env_var);
                return Some(key);
            }
        }
    }

    // 2. Fall back to OS keychain
    if let Some(key) = keychain::get_key(provider_name) {
        debug!("Found API key for {} from OS keychain", provider_name);
        return Some(key);
    }

    warn!("No API key found for provider {}", provider_name);
    None
}

/// Get the resolved provider configuration including the API key.
///
/// Returns a tuple of (model, api_key, base_url) for the specified provider.
pub fn get_provider_config(config: &D3vxConfig) -> (String, Option<String>, Option<String>) {
    let provider_name = &config.provider;
    let model = config.model.clone();

    let api_key = get_api_key(provider_name, config);

    let mut base_url = config
        .providers
        .configs
        .as_ref()
        .and_then(|configs| configs.get(provider_name))
        .and_then(|pc| pc.base_url.clone());

    if base_url.is_none() {
        base_url = SUPPORTED_PROVIDERS
            .get(provider_name)
            .and_then(|p| p.base_url)
            .map(String::from);
    }

    (model, api_key, base_url)
}
