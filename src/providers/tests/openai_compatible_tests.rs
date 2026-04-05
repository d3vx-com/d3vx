//! Tests for OpenAI-Compatible Provider
//!
//! Covers configuration, request building, error handling, and provider factory functions.

#[cfg(test)]
mod tests {
    use crate::providers::openai_compatible::{
        deepseek_config, groq_config, mistral_config, ollama_config, openai_config,
        openrouter_config, xai_config, OpenAICompatibleConfig, OpenAICompatibleProvider,
    };
    use crate::providers::Provider;

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    fn assert_valid_config(
        config: &OpenAICompatibleConfig,
        expected_id: &str,
        expected_name: &str,
    ) {
        assert_eq!(config.id, expected_id);
        assert_eq!(config.name, expected_name);
        assert!(!config.base_url.is_empty());
        assert!(config.timeout_ms > 0);
    }

    #[test]
    fn test_openai_config_creation() {
        let config = openai_config("test-key".to_string(), None);
        assert_valid_config(&config, "openai", "OpenAI");
        assert!(config.requires_api_key);
        assert!(!config.models.is_empty());

        // Check for expected models
        let model_ids: Vec<&str> = config.models.iter().map(|m| m.id).collect();
        assert!(model_ids.contains(&"gpt-4o"));
        assert!(model_ids.contains(&"gpt-4o-mini"));
        assert!(model_ids.contains(&"o3-mini"));
    }

    #[test]
    fn test_openai_config_custom_base_url() {
        let custom_url = "https://custom.openai.example.com/v1";
        let config = openai_config("test-key".to_string(), Some(custom_url.to_string()));
        assert_eq!(config.base_url, custom_url);
    }

    #[test]
    fn test_groq_config_creation() {
        let config = groq_config("test-key".to_string(), None);
        assert_valid_config(&config, "groq", "Groq");
        assert!(config.requires_api_key);
        assert!(!config.models.is_empty());

        let model_ids: Vec<&str> = config.models.iter().map(|m| m.id).collect();
        assert!(model_ids.contains(&"llama-3.3-70b-versatile"));
    }

    #[test]
    fn test_xai_config_creation() {
        let config = xai_config("test-key".to_string(), None);
        assert_valid_config(&config, "xai", "xAI");
        assert!(config.requires_api_key);

        let model_ids: Vec<&str> = config.models.iter().map(|m| m.id).collect();
        assert!(model_ids.contains(&"grok-3"));
        assert!(model_ids.contains(&"grok-3-mini"));
    }

    #[test]
    fn test_mistral_config_creation() {
        let config = mistral_config("test-key".to_string(), None);
        assert_valid_config(&config, "mistral", "Mistral");
        assert!(config.requires_api_key);

        let model_ids: Vec<&str> = config.models.iter().map(|m| m.id).collect();
        assert!(model_ids.contains(&"mistral-large-latest"));
        assert!(model_ids.contains(&"codestral-latest"));
    }

    #[test]
    fn test_deepseek_config_creation() {
        let config = deepseek_config("test-key".to_string(), None);
        assert_valid_config(&config, "deepseek", "DeepSeek");
        assert!(config.requires_api_key);

        let model_ids: Vec<&str> = config.models.iter().map(|m| m.id).collect();
        assert!(model_ids.contains(&"deepseek-chat"));
        assert!(model_ids.contains(&"deepseek-reasoner"));
    }

    #[test]
    fn test_ollama_config_creation() {
        let config = ollama_config(None);
        assert_valid_config(&config, "ollama", "Ollama (Local)");
        assert!(!config.requires_api_key); // Ollama doesn't require API key
        assert!(config.api_key.is_empty());
        assert!(config.models.is_empty()); // Models discovered dynamically
    }

    #[test]
    fn test_ollama_config_custom_url() {
        let custom_url = "http://192.168.1.100:11434/v1";
        let config = ollama_config(Some(custom_url.to_string()));
        assert_eq!(config.base_url, custom_url);
    }

    #[test]
    fn test_openrouter_config_creation() {
        let config = openrouter_config("test-key".to_string(), None);
        assert_valid_config(&config, "openrouter", "OpenRouter");
        assert!(config.requires_api_key);
        assert!(!config.extra_headers.is_empty());

        // Check extra headers are set
        assert!(config.extra_headers.contains_key("HTTP-Referer"));
        assert!(config.extra_headers.contains_key("X-Title"));
    }

    // =========================================================================
    // Provider Instance Tests
    // =========================================================================

    #[test]
    fn test_provider_creation() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_provider_models() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        let models = provider.models();
        assert!(!models.is_empty());

        let model_ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(model_ids.contains(&"gpt-4o"));
    }

    #[test]
    fn test_provider_model_info_known_model() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        let info = provider.model_info("gpt-4o");
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.id, "gpt-4o");
        assert_eq!(info.name, "GPT-4o");
        assert!(info.supports_tool_use);
        assert!(info.supports_vision);
    }

    #[test]
    fn test_provider_model_info_unknown_model() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        // Unknown models should return sensible defaults
        let info = provider.model_info("unknown-model-xyz");
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.id, "unknown-model-xyz");
        assert!(info.supports_tool_use); // Default to true for tool use
        assert!(info.supports_streaming); // Default to true
    }

    #[test]
    fn test_provider_is_available_with_key() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        assert!(provider.is_available());
    }

    #[test]
    fn test_provider_is_available_without_key() {
        let config = openai_config(String::new(), None);
        let provider = OpenAICompatibleProvider::new(config);

        assert!(!provider.is_available());
    }

    #[test]
    fn test_ollama_provider_is_available_without_key() {
        let config = ollama_config(None);
        let provider = OpenAICompatibleProvider::new(config);

        // Ollama should be available even without an API key
        assert!(provider.is_available());
    }

    #[test]
    fn test_provider_token_counting() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        // Standard approximation: ~4 chars per token
        let text = "This is a test sentence for token counting.";
        let count = provider.count_tokens(text, None);

        // Should be approximately text.len() / 4
        assert!(count > 0);
        assert!(count < text.len()); // Token count should be less than char count
    }

    #[test]
    fn test_provider_cost_estimation() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        use crate::providers::TokenUsage;

        let usage = TokenUsage {
            input_tokens: 1_000_000, // 1M tokens
            output_tokens: 500_000,  // 500K tokens
            reasoning_tokens: 0,
            cache_read_tokens: None,
            cache_write_tokens: None,
        };

        let estimate = provider.estimate_cost("gpt-4o", &usage);
        assert!(estimate.is_some());

        let estimate = estimate.unwrap();
        // GPT-4o: $2.50/MTok input, $10.00/MTok output
        // Expected: 2.5 + 5.0 = $7.50
        assert!((estimate.total_cost - 7.5).abs() < 0.01);
    }

    #[test]
    fn test_provider_cost_estimation_unknown_model() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        use crate::providers::TokenUsage;

        let usage = TokenUsage {
            input_tokens: 1_000,
            output_tokens: 500,
            reasoning_tokens: 0,
            cache_read_tokens: None,
            cache_write_tokens: None,
        };

        // Unknown models don't have pricing info
        let estimate = provider.estimate_cost("unknown-model", &usage);
        assert!(estimate.is_none());
    }

    // =========================================================================
    // Model Definition Tests
    // =========================================================================

    #[test]
    fn test_model_def_context_window() {
        let config = openai_config("test-key".to_string(), None);

        let gpt4o = config.models.iter().find(|m| m.id == "gpt-4o").unwrap();
        assert_eq!(gpt4o.context_window, 128_000);
        assert_eq!(gpt4o.max_output_tokens, 16_384);
    }

    #[test]
    fn test_model_def_thinking_support() {
        let config = openai_config("test-key".to_string(), None);

        // o3-mini supports thinking
        let o3_mini = config.models.iter().find(|m| m.id == "o3-mini").unwrap();
        assert!(o3_mini.supports_thinking);

        // gpt-4o doesn't support thinking
        let gpt4o = config.models.iter().find(|m| m.id == "gpt-4o").unwrap();
        assert!(!gpt4o.supports_thinking);
    }

    #[test]
    fn test_deepseek_reasoner_no_tool_use() {
        let config = deepseek_config("test-key".to_string(), None);

        // deepseek-reasoner doesn't support tool use
        let reasoner = config
            .models
            .iter()
            .find(|m| m.id == "deepseek-reasoner")
            .unwrap();
        assert!(!reasoner.supports_tool_use);
    }

    // =========================================================================
    // Timeout Tests
    // =========================================================================

    #[test]
    fn test_provider_timeouts() {
        // OpenAI default timeout
        let openai = openai_config("key".to_string(), None);
        assert_eq!(openai.timeout_ms, 300_000); // 5 minutes

        // Groq has shorter timeout
        let groq = groq_config("key".to_string(), None);
        assert_eq!(groq.timeout_ms, 120_000); // 2 minutes

        // Ollama has longer timeout (local models can be slow)
        let ollama = ollama_config(None);
        assert_eq!(ollama.timeout_ms, 600_000); // 10 minutes
    }
}
