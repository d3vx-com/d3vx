//! OpenAI-Compatible Provider Implementation
//!
//! A single, reusable provider implementation for any LLM API that follows
//! the OpenAI Chat Completions SSE protocol. This covers:
//!
//! - OpenAI (gpt-4o, o3-mini, etc.)
//! - Groq (llama, mixtral, etc.)
//! - xAI (grok-3, etc.)
//! - Mistral (mistral-large, codestral, etc.)
//! - DeepSeek (deepseek-chat, deepseek-reasoner, etc.)
//! - Together AI (meta-llama, etc.)
//! - Perplexity (sonar models)
//! - Ollama (local models via OpenAI-compatible endpoint)
//! - Any OpenAI-compatible proxy
//!
//! # Design (DRY/KISS)
//!
//! Instead of duplicating the HTTP + SSE logic for each provider, we implement
//! `Provider` once and configure it per-provider via `OpenAICompatibleConfig`.

pub mod streaming;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error};

use crate::providers::{
    CostEstimate, MessagesRequest, ModelInfo, Provider, ProviderError, TokenUsage,
};

pub use streaming::OpenAISseParser;

// ============================================================================
// Configuration
// ============================================================================

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

use crate::providers::ComplexityTier;

/// A model definition used to populate the model registry.
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: ComplexityTier,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub supports_tool_use: bool,
    pub supports_vision: bool,
    pub supports_thinking: bool,
    pub cost_per_input_mtok: Option<f64>,
    pub cost_per_output_mtok: Option<f64>,
}

// ============================================================================
// Built-in Provider Presets
// ============================================================================

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
                tier: ComplexityTier::Standard,
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
                tier: ComplexityTier::Simple,
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
                tier: ComplexityTier::Simple,
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
                tier: ComplexityTier::Complex,
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
                tier: ComplexityTier::Simple,
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
                tier: ComplexityTier::Standard,
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
                tier: ComplexityTier::Simple,
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
                tier: ComplexityTier::Complex,
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
                tier: ComplexityTier::Simple,
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
                tier: ComplexityTier::Complex,
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
                tier: ComplexityTier::Standard,
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
                tier: ComplexityTier::Standard,
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
                tier: ComplexityTier::Complex,
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

// ============================================================================
// Provider Implementation
// ============================================================================

/// A reusable provider for any OpenAI Chat Completions-compatible API.
pub struct OpenAICompatibleProvider {
    client: Client,
    config: OpenAICompatibleConfig,
    models: HashMap<String, ModelInfo>,
}

impl OpenAICompatibleProvider {
    /// Create a new provider from the given config.
    pub fn new(config: OpenAICompatibleConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut models = HashMap::new();
        for def in &config.models {
            models.insert(
                def.id.to_string(),
                ModelInfo {
                    id: def.id.to_string(),
                    name: def.name.to_string(),
                    provider: config.id.clone(),
                    tier: def.tier,
                    context_window: def.context_window,
                    max_output_tokens: def.max_output_tokens,
                    supports_tool_use: def.supports_tool_use,
                    supports_vision: def.supports_vision,
                    supports_streaming: true,
                    supports_thinking: def.supports_thinking,
                    cost_per_input_mtok: def.cost_per_input_mtok,
                    cost_per_output_mtok: def.cost_per_output_mtok,
                    default_thinking_budget: None,
                },
            );
        }

        Self {
            client,
            config,
            models,
        }
    }

    /// Build the request body for the OpenAI Chat Completions API.
    fn build_request_body(&self, request: &MessagesRequest) -> serde_json::Value {
        let model_info = self.model_info(&request.model);
        let max_tokens = request.max_tokens.unwrap_or_else(|| {
            model_info
                .map(|m| m.max_output_tokens as u32)
                .unwrap_or(4096)
        });

        // Convert messages to OpenAI format
        let messages: Vec<serde_json::Value> = {
            let mut msgs = Vec::new();

            // System prompt as first message
            if let Some(ref system) = request.system_prompt {
                msgs.push(serde_json::json!({
                    "role": "system",
                    "content": system,
                }));
            }

            // Conversation messages
            for msg in &request.messages {
                msgs.push(convert_message(msg));
            }

            msgs
        };

        // Convert tools
        let tools: Option<Vec<serde_json::Value>> = if request.tools.is_empty() {
            None
        } else {
            Some(request.tools.iter().map(convert_tool).collect())
        };

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": max_tokens,
            "stream": true,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        body
    }

    /// Classify HTTP errors.
    fn classify_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 => ProviderError::AuthFailed(format!("Authentication failed: {}", body)),
            429 => ProviderError::RateLimited(format!("Rate limited: {}", body)),
            400 => {
                if body.contains("context") || body.contains("token") {
                    ProviderError::ContextExceeded(format!("Context window exceeded: {}", body))
                } else {
                    ProviderError::InvalidRequest(format!("Invalid request: {}", body))
                }
            }
            500 | 502 | 503 | 529 => {
                ProviderError::Unavailable(format!("Server error {}: {}", status, body))
            }
            _ => ProviderError::StreamError(format!("HTTP {}: {}", status, body)),
        }
    }
}

#[async_trait]
impl Provider for OpenAICompatibleProvider {
    async fn send(
        &self,
        request: MessagesRequest,
    ) -> Result<crate::providers::StreamResult, ProviderError> {
        let body = self.build_request_body(&request);

        debug!(
            provider = %self.config.id,
            model = %request.model,
            message_count = request.messages.len(),
            tool_count = request.tools.len(),
            "Sending request"
        );

        let mut req_builder = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("content-type", "application/json");

        // Add auth header only if API key is present
        if !self.config.api_key.is_empty() {
            req_builder =
                req_builder.header("authorization", format!("Bearer {}", self.config.api_key));
        }

        // Add extra headers
        for (key, value) in &self.config.extra_headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder.json(&body).send().await?;

        let status = response.status();
        debug!(status = %status, provider = %self.config.id, "Received response");

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!(status = %status, body = %error_body, provider = %self.config.id, "Request failed");
            return Err(self.classify_error(status.as_u16(), &error_body));
        }

        // Parse the SSE stream through the OpenAI parser
        let parser = std::sync::Arc::new(std::sync::Mutex::new(OpenAISseParser::new()));

        let stream = response
            .bytes_stream()
            .flat_map(move |chunk_result| match chunk_result {
                Ok(bytes) => {
                    let parser = parser.clone();
                    let events = {
                        let mut p = parser.lock().unwrap_or_else(|e| e.into_inner());
                        p.parse(&bytes)
                    };
                    futures::stream::iter(events.into_iter().map(Ok).collect::<Vec<_>>())
                }
                Err(e) => {
                    let error = if e.is_timeout() {
                        ProviderError::Timeout
                    } else if e.is_connect() {
                        ProviderError::Network(e.to_string())
                    } else {
                        ProviderError::StreamError(e.to_string())
                    };
                    futures::stream::iter(vec![Err(error)])
                }
            });

        Ok(Box::pin(stream))
    }

    fn name(&self) -> &str {
        &self.config.id
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.models.values().cloned().collect()
    }

    fn model_info(&self, model_id: &str) -> Option<ModelInfo> {
        self.models.get(model_id).cloned().or_else(|| {
            // Return sensible defaults for unknown models (user may pass any model ID)
            Some(ModelInfo {
                id: model_id.to_string(),
                name: model_id.to_string(),
                provider: self.config.id.clone(),
                tier: ComplexityTier::Standard,
                context_window: 128_000,
                max_output_tokens: 4_096,
                supports_tool_use: true,
                supports_vision: false,
                supports_streaming: true,
                supports_thinking: false,
                cost_per_input_mtok: None,
                cost_per_output_mtok: None,
                default_thinking_budget: None,
            })
        })
    }

    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        match self.config.id.as_str() {
            "ollama" => {
                let base = self
                    .config
                    .base_url
                    .trim_end_matches("/v1")
                    .trim_end_matches('/');
                self.discover_ollama(base).await
            }
            "openrouter" => {
                self.discover_openrouter(&self.config.base_url, &self.config.api_key)
                    .await
            }
            _ => Ok(self.models()),
        }
    }

    fn count_tokens(&self, text: &str, _model: Option<&str>) -> usize {
        // ~4 chars per token (standard approximation for most LLM tokenizers)
        text.len() / 4
    }

    fn is_available(&self) -> bool {
        if self.config.requires_api_key {
            !self.config.api_key.is_empty()
        } else {
            true // Ollama and other local providers don't need a key
        }
    }

    fn estimate_cost(&self, model: &str, usage: &TokenUsage) -> Option<CostEstimate> {
        let info = self.model_info(model)?;
        let input_rate = info.cost_per_input_mtok?;
        let output_rate = info.cost_per_output_mtok?;

        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * output_rate;

        Some(CostEstimate {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            currency: "USD".to_string(),
        })
    }

    async fn discover_ollama(&self, _base_url: &str) -> Result<Vec<ModelInfo>, ProviderError> {
        let base = self
            .config
            .base_url
            .trim_end_matches("/v1")
            .trim_end_matches('/');
        let url = format!("{}/api/tags", base);

        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(ProviderError::Unavailable(format!(
                "Ollama API returned {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response.json().await?;
        let mut models = Vec::new();

        if let Some(models_array) = json["models"].as_array() {
            for m in models_array {
                if let Some(name) = m["name"].as_str() {
                    models.push(ModelInfo {
                        id: name.to_string(),
                        name: name.to_string(),
                        provider: "ollama".to_string(),
                        tier: ComplexityTier::Standard,
                        context_window: 32_000,
                        max_output_tokens: 4_096,
                        supports_tool_use: true,
                        supports_vision: false,
                        supports_streaming: true,
                        supports_thinking: false,
                        cost_per_input_mtok: Some(0.0),
                        cost_per_output_mtok: Some(0.0),
                        default_thinking_budget: None,
                    });
                }
            }
        }

        Ok(models)
    }

    async fn discover_openrouter(
        &self,
        _base_url: &str,
        _api_key: &str,
    ) -> Result<Vec<ModelInfo>, ProviderError> {
        let url = format!("{}/models", self.config.base_url);
        let mut req = self.client.get(&url);
        if !self.config.api_key.is_empty() {
            req = req.header("authorization", format!("Bearer {}", self.config.api_key));
        }

        let response = req.send().await?;
        if !response.status().is_success() {
            return Err(ProviderError::Unavailable(format!(
                "OpenRouter API returned {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response.json().await?;
        let mut models = Vec::new();

        if let Some(data) = json["data"].as_array() {
            for m in data {
                if let Some(id) = m["id"].as_str() {
                    let name = m["name"].as_str().unwrap_or(id);
                    let context = m["context_length"].as_u64().unwrap_or(128_000);

                    let pricing = &m["pricing"];
                    let input_cost = pricing["prompt"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|f| f * 1000000.0);

                    let tier = if let Some(cost) = input_cost {
                        if cost > 5.0 {
                            ComplexityTier::Complex
                        } else if cost > 0.5 {
                            ComplexityTier::Standard
                        } else {
                            ComplexityTier::Simple
                        }
                    } else {
                        ComplexityTier::Standard
                    };

                    models.push(ModelInfo {
                        id: id.to_string(),
                        name: name.to_string(),
                        provider: "openrouter".to_string(),
                        tier,
                        context_window: context,
                        max_output_tokens: 4_096,
                        supports_tool_use: true,
                        supports_vision: false,
                        supports_streaming: true,
                        supports_thinking: false,
                        cost_per_input_mtok: input_cost,
                        cost_per_output_mtok: pricing["completion"]
                            .as_str()
                            .and_then(|s| s.parse::<f64>().ok())
                            .map(|f| f * 1000000.0),
                        default_thinking_budget: None,
                    });
                }
            }
        }

        Ok(models)
    }
}

// ============================================================================
// Message Conversion (OpenAI format)
// ============================================================================

fn convert_message(msg: &crate::providers::Message) -> serde_json::Value {
    use crate::providers::{ContentBlock, MessageContent, Role};

    let role = match msg.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };

    match &msg.content {
        MessageContent::Text(text) => serde_json::json!({
            "role": role,
            "content": text,
        }),
        MessageContent::Blocks(blocks) => {
            // Check if we have tool_use or tool_result blocks
            let has_tool_use = blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
            let has_tool_result = blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { .. }));

            if has_tool_result {
                // Tool results go as separate messages
                let block = blocks.iter().find_map(|b| match b {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => Some(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": content,
                    })),
                    _ => None,
                });
                block.unwrap_or_else(|| serde_json::json!({"role": role, "content": ""}))
            } else if has_tool_use {
                // Tool use blocks from assistant
                let tool_calls: Vec<serde_json::Value> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input } => Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input.to_string(),
                            }
                        })),
                        _ => None,
                    })
                    .collect();

                let text_content: String = blocks
                    .iter()
                    .filter_map(|b| b.as_text())
                    .collect::<Vec<_>>()
                    .join("");

                let mut msg = serde_json::json!({
                    "role": "assistant",
                    "tool_calls": tool_calls,
                });
                if !text_content.is_empty() {
                    msg["content"] = serde_json::json!(text_content);
                }
                msg
            } else {
                // Plain blocks — concatenate text
                let text: String = blocks
                    .iter()
                    .filter_map(|b| b.as_text())
                    .collect::<Vec<_>>()
                    .join("");
                serde_json::json!({
                    "role": role,
                    "content": text,
                })
            }
        }
    }
}

fn convert_tool(tool: &crate::providers::ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        }
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config_creation() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);
        assert_eq!(provider.name(), "openai");
        assert!(provider.is_available());
    }

    #[test]
    fn test_groq_config_creation() {
        let config = groq_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);
        assert_eq!(provider.name(), "groq");
        assert!(provider.is_available());
        assert!(provider.model_info("llama-3.3-70b-versatile").is_some());
    }

    #[test]
    fn test_ollama_no_key_required() {
        let config = ollama_config(None);
        let provider = OpenAICompatibleProvider::new(config);
        assert_eq!(provider.name(), "ollama");
        assert!(provider.is_available()); // No key needed
    }

    #[test]
    fn test_empty_key_provider_unavailable() {
        let config = openai_config(String::new(), None);
        let provider = OpenAICompatibleProvider::new(config);
        assert!(!provider.is_available());
    }

    #[test]
    fn test_unknown_model_defaults() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);
        let info = provider.model_info("future-model-2026").unwrap();
        assert_eq!(info.id, "future-model-2026");
        assert_eq!(info.context_window, 128_000);
    }

    #[test]
    fn test_cost_estimation() {
        let config = openai_config("test-key".to_string(), None);
        let provider = OpenAICompatibleProvider::new(config);

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            reasoning_tokens: 0,
            cache_read_tokens: None,
            cache_write_tokens: None,
        };

        let cost = provider.estimate_cost("gpt-4o", &usage).unwrap();
        // Input: 1M tokens at $2.5/MTok = $2.50
        // Output: 500K tokens at $10/MTok = $5.00
        assert!((cost.input_cost - 2.5).abs() < 0.01);
        assert!((cost.output_cost - 5.0).abs() < 0.01);
        assert!((cost.total_cost - 7.5).abs() < 0.01);
    }

    #[test]
    fn test_openrouter_extra_headers() {
        let config = openrouter_config("test-key".to_string(), None);
        assert!(config.extra_headers.contains_key("HTTP-Referer"));
        assert!(config.extra_headers.contains_key("X-Title"));
    }
}
