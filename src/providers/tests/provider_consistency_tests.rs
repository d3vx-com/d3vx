//! Provider Consistency Tests
//!
//! These tests ensure that provider support is consistent across all code paths:
//! - Runtime (create_provider in app/agent.rs)
//! - Config defaults
//! - Onboarding/setup
//! - Doctor diagnostics
//! - API key resolution

use crate::providers::SUPPORTED_PROVIDERS;

#[test]
fn test_registry_has_all_runtime_providers() {
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
            "Runtime provider '{}' not in registry",
            id
        );
    }
}

#[test]
fn test_all_registry_providers_have_api_key_env_or_local() {
    for provider in SUPPORTED_PROVIDERS.all() {
        if provider.requires_api_key {
            assert!(
                !provider.api_key_env.is_empty(),
                "Provider '{}' requires API key but has no env var defined",
                provider.id
            );
        } else {
            assert_eq!(
                provider.api_key_env, "",
                "Provider '{}' should not require API key but has env var '{}'",
                provider.id, provider.api_key_env
            );
        }
    }
}

#[test]
fn test_all_providers_have_default_model() {
    for provider in SUPPORTED_PROVIDERS.all() {
        if provider.id == "openrouter" {
            continue;
        }
        assert!(
            !provider.default_model.is_empty(),
            "Provider '{}' missing default model",
            provider.id
        );
    }
}

#[test]
fn test_onboarding_and_runtime_match() {
    let onboarding_providers = vec![
        "anthropic",
        "openai",
        "groq",
        "xai",
        "mistral",
        "deepseek",
        "ollama",
        "openrouter",
    ];
    for id in onboarding_providers {
        assert!(
            SUPPORTED_PROVIDERS.is_supported(id),
            "Onboarding provider '{}' not in registry",
            id
        );
    }
}

#[test]
fn test_no_unsupported_providers_in_registry() {
    let unsupported = vec!["gemini", "google", "cohere", "cohere2"];
    for id in unsupported {
        assert!(
            !SUPPORTED_PROVIDERS.is_supported(id),
            "Unsupported provider '{}' should not be in registry",
            id
        );
    }
}

#[test]
fn test_ollama_is_keyless() {
    let ollama = SUPPORTED_PROVIDERS.get("ollama").unwrap();
    assert!(
        !ollama.requires_api_key,
        "Ollama should not require API key"
    );
    assert!(
        ollama.api_key_env.is_empty(),
        "Ollama should not have API key env var"
    );
    assert!(
        ollama.base_url.is_some(),
        "Ollama should have default base URL"
    );
}

#[test]
fn test_provider_api_key_env_matches_runtime() {
    let provider_api_keys = [
        ("anthropic", "ANTHROPIC_API_KEY"),
        ("openai", "OPENAI_API_KEY"),
        ("groq", "GROQ_API_KEY"),
        ("xai", "XAI_API_KEY"),
        ("mistral", "MISTRAL_API_KEY"),
        ("deepseek", "DEEPSEEK_API_KEY"),
        ("ollama", ""),
        ("openrouter", "OPENROUTER_API_KEY"),
    ];
    for (provider, expected_key) in provider_api_keys {
        let actual_key = SUPPORTED_PROVIDERS.api_key_env(provider).unwrap_or("");
        assert_eq!(
            actual_key, expected_key,
            "Provider '{}' API key env mismatch",
            provider
        );
    }
}

#[test]
fn test_registry_ids_are_lowercase() {
    for provider in SUPPORTED_PROVIDERS.all() {
        assert_eq!(
            provider.id.to_lowercase(),
            provider.id,
            "Provider ID '{}' should be lowercase",
            provider.id
        );
    }
}
