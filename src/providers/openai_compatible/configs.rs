//! Provider preset configurations
//!
//! Factory functions for creating configs instances for each
//! OpenAI-compatible provider (OpenAI, Groq, xAI, etc.).

use super::*;

/// Configuration for an OpenAI-compatible provider instance.
#[derive(Debug, Clone)]
pub struct OpenAICompatibleConfig {
    /// Provider identifier (e.g., "openai", "groq", "ollama").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Base URL for API requests.
    pub base_url: String,
    /// API key (empty string for keyless providers like Ollama).
    pub api_key: String,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Custom headers to send with every request.
    pub extra_headers: HashMap<String, String>,
    /// Whether this provider requires an API key to function.
    pub requires_api_key: bool,
    /// Model definitions for this provider.
    pub models: Vec<ModelDef>,
}

/// A model definition used to populate the model registry.
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub supports_tool_use: bool,
    pub supports_vision: bool,
    pub supports_thinking: bool,
    pub cost_per_input_mtok: Option<f64>,
    pub cost_per_output_mtok: Option<f64>,
}

/// Create a config for the OpenAI provider.
pub fn openai_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "openai".into(),
        name: "OpenAI".into(),
        base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        api_key,
        timeout_ms: 300_000,
        extra_headers: HashMap::new(),
        requires_api_key: true,
        models: vec![
            ModelDef {
                id: "gpt-4o",
                name: "GPT-4o",
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_tool_use: true,
                supports_vision: true,
                supports_thinking: false,
                cost_per_input_mtok: Some(2.5),
                cost_per_output_mtok: Some(10.0),
            },
            ModelDef {
                id: "gpt-4o-mini",
                name: "GPT-4o Mini",
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_tool_use: true,
                supports_vision: true,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.15),
                cost_per_output_mtok: Some(0.6),
            },
            ModelDef {
                id: "o3-mini",
                name: "o3 Mini",
                context_window: 200_000,
                max_output_tokens: 100_000,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: true,
                cost_per_input_mtok: Some(1.1),
                cost_per_output_mtok: Some(4.4),
            },
            ModelDef {
                id: "gpt-4.1",
                name: "GPT-4.1",
                context_window: 1_047_576,
                max_output_tokens: 32_768,
                supports_tool_use: true,
                supports_vision: true,
                supports_thinking: false,
                cost_per_input_mtok: Some(2.0),
                cost_per_output_mtok: Some(8.0),
            },
            ModelDef {
                id: "gpt-4.1-mini",
                name: "GPT-4.1 Mini",
                context_window: 1_047_576,
                max_output_tokens: 32_768,
                supports_tool_use: true,
                supports_vision: true,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.4),
                cost_per_output_mtok: Some(1.6),
            },
        ],
    }
}

/// Create a config for the Groq provider.
pub fn groq_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "groq".into(),
        name: "Groq".into(),
        base_url: base_url.unwrap_or_else(|| "https://api.groq.com/openai/v1".into()),
        api_key,
        timeout_ms: 120_000,
        extra_headers: HashMap::new(),
        requires_api_key: true,
        models: vec![
            ModelDef {
                id: "llama-3.3-70b-versatile",
                name: "Llama 3.3 70B",
                context_window: 128_000,
                max_output_tokens: 32_768,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.59),
                cost_per_output_mtok: Some(0.79),
            },
            ModelDef {
                id: "mixtral-8x7b-32768",
                name: "Mixtral 8x7B",
                context_window: 32_768,
                max_output_tokens: 32_768,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.24),
                cost_per_output_mtok: Some(0.24),
            },
        ],
    }
}

/// Create a config for the xAI (Grok) provider.
pub fn xai_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "xai".into(),
        name: "xAI".into(),
        base_url: base_url.unwrap_or_else(|| "https://api.x.ai/v1".into()),
        api_key,
        timeout_ms: 300_000,
        extra_headers: HashMap::new(),
        requires_api_key: true,
        models: vec![
            ModelDef {
                id: "grok-3",
                name: "Grok 3",
                context_window: 131_072,
                max_output_tokens: 16_384,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(3.0),
                cost_per_output_mtok: Some(15.0),
            },
            ModelDef {
                id: "grok-3-mini",
                name: "Grok 3 Mini",
                context_window: 131_072,
                max_output_tokens: 16_384,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: true,
                cost_per_input_mtok: Some(0.3),
                cost_per_output_mtok: Some(0.5),
            },
        ],
    }
}

/// Create a config for the Mistral provider.
pub fn mistral_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "mistral".into(),
        name: "Mistral".into(),
        base_url: base_url.unwrap_or_else(|| "https://api.mistral.ai/v1".into()),
        api_key,
        timeout_ms: 300_000,
        extra_headers: HashMap::new(),
        requires_api_key: true,
        models: vec![
            ModelDef {
                id: "mistral-large-latest",
                name: "Mistral Large",
                context_window: 128_000,
                max_output_tokens: 8_192,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(2.0),
                cost_per_output_mtok: Some(6.0),
            },
            ModelDef {
                id: "codestral-latest",
                name: "Codestral",
                context_window: 256_000,
                max_output_tokens: 8_192,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.3),
                cost_per_output_mtok: Some(0.9),
            },
        ],
    }
}

/// Create a config for the DeepSeek provider.
pub fn deepseek_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "deepseek".into(),
        name: "DeepSeek".into(),
        base_url: base_url.unwrap_or_else(|| "https://api.deepseek.com/v1".into()),
        api_key,
        timeout_ms: 300_000,
        extra_headers: HashMap::new(),
        requires_api_key: true,
        models: vec![
            ModelDef {
                id: "deepseek-chat",
                name: "DeepSeek V3",
                context_window: 64_000,
                max_output_tokens: 8_192,
                supports_tool_use: true,
                supports_vision: false,
                supports_thinking: false,
                cost_per_input_mtok: Some(0.27),
                cost_per_output_mtok: Some(1.1),
            },
            ModelDef {
                id: "deepseek-reasoner",
                name: "DeepSeek R1",
                context_window: 64_000,
                max_output_tokens: 8_192,
                supports_tool_use: false,
                supports_vision: false,
                supports_thinking: true,
                cost_per_input_mtok: Some(0.55),
                cost_per_output_mtok: Some(2.19),
            },
        ],
    }
}

/// Create a config for the Ollama provider (local).
pub fn ollama_config(base_url: Option<String>) -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        id: "ollama".into(),
        name: "Ollama (Local)".into(),
        base_url: base_url.unwrap_or_else(|| "http://localhost:11434/v1".into()),
        api_key: String::new(),
        timeout_ms: 600_000,
        extra_headers: HashMap::new(),
        requires_api_key: false,
        models: vec![], // Ollama models are discovered dynamically
    }
}

/// Create a config for OpenRouter.
pub fn openrouter_config(api_key: String, base_url: Option<String>) -> OpenAICompatibleConfig {
    let mut headers = HashMap::new();
    headers.insert("HTTP-Referer".to_string(), "https://d3vx.dev".to_string());
    headers.insert("X-Title".to_string(), "d3vx".to_string());
    OpenAICompatibleConfig {
        id: "openrouter".into(),
        name: "OpenRouter".into(),
        base_url: base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
        api_key,
        timeout_ms: 300_000,
        extra_headers: headers,
        requires_api_key: true,
        models: vec![], // OpenRouter proxies all models; user specifies model ID directly
    }
}
