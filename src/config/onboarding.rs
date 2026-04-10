//! Onboarding and Setup Utilities
//!
//! Provides first-run detection, setup wizards, and helpful error messages
//! to guide users through initial configuration.

use crate::config::defaults::get_global_config_path;
use crate::config::keychain;
use crate::providers::SUPPORTED_PROVIDERS;
use std::env;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct OnboardingStatus {
    pub is_first_run: bool,
    pub has_config: bool,
    pub has_api_key: bool,
    pub missing_provider: Option<String>,
    pub provider_api_key_env: String,
    /// Config exists but no API key found — setup is recommended
    pub needs_api_key_setup: bool,
}

#[allow(dead_code)]
pub fn supported_providers() -> Vec<(&'static str, &'static str, &'static str)> {
    SUPPORTED_PROVIDERS.to_tuple_list()
}

pub fn check_onboarding_status() -> OnboardingStatus {
    let config_path = get_global_config_path();
    let has_config = Path::new(&config_path).exists();

    let provider = env::var("D3VX_PROVIDER")
        .ok()
        .unwrap_or_else(|| "anthropic".to_string());

    let provider_info = SUPPORTED_PROVIDERS.get(&provider);
    let provider_api_key_env = provider_info.map(|p| p.api_key_env).unwrap_or("");

    // Check env var first, then fall back to OS keychain
    let has_api_key = if provider_api_key_env.is_empty() {
        true // Provider doesn't need a key (e.g. Ollama)
    } else {
        env::var(provider_api_key_env)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || keychain::has_key(&provider)
    };

    let is_first_run = !has_config && !has_api_key;
    // Broader check: config exists but API key is not set
    let needs_api_key_setup = has_config && !has_api_key;

    OnboardingStatus {
        is_first_run,
        has_config,
        has_api_key,
        missing_provider: if has_api_key { None } else { Some(provider) },
        provider_api_key_env: provider_api_key_env.to_string(),
        needs_api_key_setup,
    }
}

pub fn get_setup_instructions(provider: &str) -> String {
    match provider {
        "anthropic" => r#"
To get started with d3vx, you need to set your Anthropic API key:

1. Get your API key from https://console.anthropic.com/settings/keys
2. Set the environment variable:
   
   export ANTHROPIC_API_KEY="sk-ant-..."

3. Run d3vx again

For permanent setup, add the export line to your shell profile (~/.zshrc, ~/.bashrc)
"#
        .to_string(),
        "openai" => r#"
To get started with d3vx, you need to set your OpenAI API key:

1. Get your API key from https://platform.openai.com/api-keys
2. Set the environment variable:
   
   export OPENAI_API_KEY="sk-..."

3. Run d3vx again

For permanent setup, add the export line to your shell profile (~/.zshrc, ~/.bashrc)
"#
        .to_string(),
        "ollama" => r#"
Ollama is a local LLM provider. To use it:

1. Install Ollama from https://ollama.ai
2. Pull a model:  ollama pull qwen2.5-coder:32b
3. Start the server: ollama serve
4. Set environment variable (optional):
   
   export OLLAMA_HOST="http://localhost:11434"

5. Run d3vx again
"#
        .to_string(),
        _ => format!(
            r#"
To get started with d3vx, you need to set your {} API key.

Check the provider documentation for how to obtain an API key,
then set the appropriate environment variable.

Run `d3vx doctor` for environment diagnostics.
"#,
            provider
        ),
    }
}

pub fn format_provider_options() -> String {
    use crate::providers::SUPPORTED_PROVIDERS;

    let mut output = String::from("Supported providers:\n");
    for provider in SUPPORTED_PROVIDERS.all() {
        let marker = if provider.id == "anthropic" {
            " (default)"
        } else {
            ""
        };
        output.push_str(&format!(
            "  {:<12} - {}{}\n",
            provider.id, provider.name, marker
        ));
    }
    output.push_str("\nUse --provider <name> to select a different provider.");
    output
}

pub fn get_doctor_command_hint() -> &'static str {
    "Run `d3vx doctor` to check your environment setup."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::SUPPORTED_PROVIDERS;

    #[test]
    fn test_supported_providers_cover_default() {
        assert!(
            SUPPORTED_PROVIDERS.is_supported("anthropic"),
            "Default provider should be supported"
        );
        assert!(
            SUPPORTED_PROVIDERS.is_supported("ollama"),
            "Local provider should be supported"
        );
    }

    #[test]
    fn test_get_setup_instructions_for_all_providers() {
        for provider in SUPPORTED_PROVIDERS.all() {
            let instructions = get_setup_instructions(provider.id);
            assert!(
                !instructions.is_empty(),
                "Setup instructions for {} should not be empty",
                provider.id
            );
        }
    }

    #[test]
    fn test_onboarding_status_structure() {
        let status = check_onboarding_status();
        assert_eq!(
            status.provider_api_key_env.is_empty(),
            status.missing_provider.as_deref() == Some("ollama")
        );
    }

    #[test]
    fn test_supported_providers_matches_registry() {
        let providers: Vec<&str> = supported_providers().iter().map(|(id, _, _)| *id).collect();
        for provider in SUPPORTED_PROVIDERS.all() {
            assert!(
                providers.contains(&provider.id),
                "Provider {} in registry but not in onboarding",
                provider.id
            );
        }
    }
}
