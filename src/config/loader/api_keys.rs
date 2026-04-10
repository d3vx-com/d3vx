//! API key resolution and provider configuration lookup

use super::super::types::D3vxConfig;
use crate::config::auth;
use crate::providers::SUPPORTED_PROVIDERS;
use tracing::{debug, warn};

/// Get the API key for a specific provider.
///
/// Resolution order:
/// 1. Stored credential in `~/.d3vx/auth.json` (set via `d3vx setup`)
pub fn get_api_key(provider_name: &str, _config: &D3vxConfig) -> Option<String> {
    if let Some(key) = auth::get_key(provider_name) {
        debug!("Found API key for {} from auth.json", provider_name);
        return Some(key);
    }

    // Provider doesn't require a key (e.g. Ollama)
    let provider_info = SUPPORTED_PROVIDERS.get(provider_name);
    if let Some(info) = provider_info {
        if !info.requires_api_key {
            debug!("Provider {} does not require an API key", provider_name);
            return None;
        }
    }

    warn!(
        "No API key found for provider {}. Run: d3vx setup",
        provider_name
    );
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
