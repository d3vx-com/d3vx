//! Provider Registry — single source of truth for supported LLM providers.
//!
//! Every supported provider is described by one `ProviderInfo` entry in
//! [`PROVIDER_INFOS`] below. Adding a provider = add one struct to that
//! array. The registry API (lookup, iteration, capability queries) is
//! built from this list.
//!
//! Previously each provider was registered via a separate
//! `providers.insert(...)` call inside a 100-line constructor — a
//! copy-paste invitation that made the "list of supported providers"
//! implicitly distributed across 8 insertion blocks. A single static
//! array is shorter, obviously correct, and keeps the canonical list
//! visible in one screen.
//!
//! # Supported Providers
//!
//! | Provider    | Default Model              | API Key Env        | Requires Key |
//! |-------------|----------------------------|--------------------|--------------|
//! | anthropic   | claude-sonnet-4-20250514   | ANTHROPIC_API_KEY  | Yes          |
//! | openai      | gpt-4o                     | OPENAI_API_KEY     | Yes          |
//! | groq        | llama-3.3-70b-versatile    | GROQ_API_KEY       | Yes          |
//! | xai         | grok-3                     | XAI_API_KEY        | Yes          |
//! | mistral     | mistral-large-latest       | MISTRAL_API_KEY    | Yes          |
//! | deepseek    | deepseek-chat              | DEEPSEEK_API_KEY   | Yes          |
//! | ollama      | qwen2.5-coder:32b          | (none - local)     | No           |
//! | openrouter  | (dynamic)                  | OPENROUTER_API_KEY | Yes          |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

pub static SUPPORTED_PROVIDERS: LazyLock<ProviderRegistry> =
    LazyLock::new(ProviderRegistry::new);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub api_key_env: &'static str,
    pub requires_api_key: bool,
    pub default_model: &'static str,
    pub cheap_model: Option<&'static str>,
    pub base_url: Option<&'static str>,
    pub timeout_ms: u64,
}

/// Canonical list of supported providers.
///
/// To add a new provider, append one `ProviderInfo` entry here. Nothing
/// else in this file needs to change; downstream code iterates via
/// [`ProviderRegistry::all`] and looks up via [`ProviderRegistry::get`].
const PROVIDER_INFOS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "anthropic",
        name: "Anthropic Claude",
        api_key_env: "ANTHROPIC_API_KEY",
        requires_api_key: true,
        default_model: "claude-sonnet-4-20250514",
        cheap_model: Some("claude-haiku-4-5-20251001"),
        base_url: None,
        timeout_ms: 300_000,
    },
    ProviderInfo {
        id: "openai",
        name: "OpenAI GPT",
        api_key_env: "OPENAI_API_KEY",
        requires_api_key: true,
        default_model: "gpt-4o",
        cheap_model: Some("gpt-4o-mini"),
        base_url: None,
        timeout_ms: 300_000,
    },
    ProviderInfo {
        id: "groq",
        name: "Groq",
        api_key_env: "GROQ_API_KEY",
        requires_api_key: true,
        default_model: "llama-3.3-70b-versatile",
        cheap_model: Some("mixtral-8x7b-32768"),
        base_url: None,
        timeout_ms: 120_000,
    },
    ProviderInfo {
        id: "xai",
        name: "xAI Grok",
        api_key_env: "XAI_API_KEY",
        requires_api_key: true,
        default_model: "grok-3",
        cheap_model: Some("grok-3-mini"),
        base_url: None,
        timeout_ms: 300_000,
    },
    ProviderInfo {
        id: "mistral",
        name: "Mistral",
        api_key_env: "MISTRAL_API_KEY",
        requires_api_key: true,
        default_model: "mistral-large-latest",
        cheap_model: Some("codestral-latest"),
        base_url: None,
        timeout_ms: 300_000,
    },
    ProviderInfo {
        id: "deepseek",
        name: "DeepSeek",
        api_key_env: "DEEPSEEK_API_KEY",
        requires_api_key: true,
        default_model: "deepseek-chat",
        cheap_model: Some("deepseek-chat"),
        base_url: None,
        timeout_ms: 300_000,
    },
    ProviderInfo {
        id: "ollama",
        name: "Ollama (Local)",
        api_key_env: "",
        requires_api_key: false,
        default_model: "qwen2.5-coder:32b",
        cheap_model: None,
        base_url: Some("http://localhost:11434"),
        timeout_ms: 600_000,
    },
    ProviderInfo {
        id: "openrouter",
        name: "OpenRouter",
        api_key_env: "OPENROUTER_API_KEY",
        requires_api_key: true,
        default_model: "",
        cheap_model: None,
        base_url: None,
        timeout_ms: 300_000,
    },
];

pub struct ProviderRegistry {
    providers: HashMap<&'static str, ProviderInfo>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let providers = PROVIDER_INFOS
            .iter()
            .cloned()
            .map(|p| (p.id, p))
            .collect();
        Self { providers }
    }

    pub fn get(&self, id: &str) -> Option<&ProviderInfo> {
        self.providers.get(id)
    }

    pub fn is_supported(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &&'static str> {
        self.providers.keys()
    }

    pub fn all(&self) -> impl Iterator<Item = &ProviderInfo> {
        self.providers.values()
    }

    pub fn requires_key(&self, id: &str) -> bool {
        self.get(id).map(|p| p.requires_api_key).unwrap_or(false)
    }

    pub fn api_key_env(&self, id: &str) -> Option<&'static str> {
        self.get(id)
            .map(|p| p.api_key_env)
            .filter(|s| !s.is_empty())
    }

    pub fn default_model(&self, id: &str) -> Option<&'static str> {
        self.get(id)
            .map(|p| p.default_model)
            .filter(|s| !s.is_empty())
    }

    pub fn to_tuple_list(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        self.providers
            .values()
            .map(|p| (p.id, p.api_key_env, p.name))
            .collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_every_declared_provider() {
        // The const list is the canonical source; the registry must
        // expose everything in it. Previously a hardcoded "runtime"
        // list in the test file enforced this coupling manually —
        // that list is now the const, so the test is trivially true
        // *and* self-updating when providers are added.
        assert_eq!(
            SUPPORTED_PROVIDERS.providers.len(),
            PROVIDER_INFOS.len(),
            "registry must expose every entry in PROVIDER_INFOS"
        );
        for info in PROVIDER_INFOS {
            assert!(
                SUPPORTED_PROVIDERS.is_supported(info.id),
                "provider {} declared in PROVIDER_INFOS but missing from registry",
                info.id
            );
        }
    }

    #[test]
    fn provider_ids_are_unique() {
        // A duplicate id would silently overwrite the first entry when
        // building the HashMap. Guard against that here.
        let mut ids: Vec<&str> = PROVIDER_INFOS.iter().map(|p| p.id).collect();
        ids.sort();
        let original_len = ids.len();
        ids.dedup();
        assert_eq!(
            ids.len(),
            original_len,
            "duplicate provider id in PROVIDER_INFOS"
        );
    }

    #[test]
    fn all_providers_have_required_fields() {
        for provider in PROVIDER_INFOS {
            assert!(
                !provider.name.is_empty(),
                "Provider {} missing name",
                provider.id
            );
            assert!(
                !provider.id.is_empty(),
                "Provider has empty id",
            );
            if provider.requires_api_key {
                assert!(
                    !provider.api_key_env.is_empty(),
                    "Provider {} requires API key but has empty env var",
                    provider.id
                );
            } else {
                // Non-key providers (ollama) must document their local endpoint.
                assert!(
                    provider.base_url.is_some(),
                    "Provider {} requires no key but has no base_url",
                    provider.id
                );
            }
        }
    }

    #[test]
    fn ollama_is_keyless_and_local() {
        let ollama = SUPPORTED_PROVIDERS.get("ollama").unwrap();
        assert!(!ollama.requires_api_key);
        assert!(ollama.api_key_env.is_empty());
        assert!(ollama.base_url.is_some());
    }

    #[test]
    fn canonical_providers_are_supported() {
        // Smoke-check a few long-term provider ids that external code
        // and user config explicitly reference.
        for id in ["anthropic", "openai", "groq", "ollama"] {
            assert!(
                SUPPORTED_PROVIDERS.is_supported(id),
                "canonical provider {} missing",
                id
            );
        }
    }
}
