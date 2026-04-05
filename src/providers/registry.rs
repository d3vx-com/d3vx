//! Provider Registry - Single Source of Truth
//!
//! This module defines the canonical list of supported LLM providers.
//! All code paths (config, onboarding, runtime, doctor) should reference
//! this registry to ensure consistency.
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
    LazyLock::new(|| ProviderRegistry::new());

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

pub struct ProviderRegistry {
    providers: HashMap<&'static str, ProviderInfo>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut providers = HashMap::new();

        providers.insert(
            "anthropic",
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
        );

        providers.insert(
            "openai",
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
        );

        providers.insert(
            "groq",
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
        );

        providers.insert(
            "xai",
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
        );

        providers.insert(
            "mistral",
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
        );

        providers.insert(
            "deepseek",
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
        );

        providers.insert(
            "ollama",
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
        );

        providers.insert(
            "openrouter",
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
        );

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
    fn test_all_providers_have_required_fields() {
        for provider in SUPPORTED_PROVIDERS.all() {
            assert!(
                !provider.name.is_empty(),
                "Provider {} missing name",
                provider.id
            );
            assert!(
                provider.id == "ollama" || !provider.api_key_env.is_empty(),
                "Provider {} missing API key env",
                provider.id
            );
            if provider.requires_api_key {
                assert!(
                    !provider.api_key_env.is_empty(),
                    "Provider {} requires API key but has empty env var",
                    provider.id
                );
            }
        }
    }

    #[test]
    fn test_ollama_no_api_key() {
        let ollama = SUPPORTED_PROVIDERS.get("ollama").unwrap();
        assert!(!ollama.requires_api_key);
        assert!(ollama.api_key_env.is_empty());
    }

    #[test]
    fn test_default_providers_exist() {
        for id in ["anthropic", "openai", "groq", "ollama"] {
            assert!(
                SUPPORTED_PROVIDERS.is_supported(id),
                "Canonical provider {} missing",
                id
            );
        }
    }

    #[test]
    fn test_providers_match_runtime() {
        let runtime_providers = vec![
            "anthropic",
            "openai",
            "groq",
            "xai",
            "mistral",
            "deepseek",
            "ollama",
            "openrouter",
        ];
        for id in runtime_providers {
            assert!(
                SUPPORTED_PROVIDERS.is_supported(id),
                "Runtime provider {} not in registry",
                id
            );
        }
    }
}
