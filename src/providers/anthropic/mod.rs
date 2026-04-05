//! Anthropic (Claude) Provider Implementation
//!
//! This module implements the [`Provider`] trait for Anthropic's Claude API.
//!
//! # Features
//!
//! - Full streaming with tool_use support
//! - Extended thinking (thinking blocks)
//! - Prompt caching (cache_control)
//! - Retry with exponential backoff on 429/529
//!
//! # Usage
//!
//! ```ignore
//! use d3vx::providers::anthropic::AnthropicProvider;
//! use d3vx::providers::{Provider, MessagesRequest};
//!
//! let provider = AnthropicProvider::new("sk-ant-...".to_string());
//! let stream = provider.send(request).await?;
//! ```

mod streaming;
mod types;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error};

pub use streaming::parse_anthropic_event;
pub use streaming::SseParser;
pub use types::*;

use crate::providers::{
    CostEstimate, MessagesRequest, ModelInfo, Provider, ProviderError, ProviderOptions,
    StreamEvent, TokenUsage,
};

// ============================================================================
// Model Registry
// ============================================================================

use crate::providers::ComplexityTier;

/// Anthropic model definitions.
const ANTHROPIC_MODELS: &[(
    &str,
    &str,
    ComplexityTier,
    u64,
    u64,
    f64,
    f64,
    bool,
    Option<u32>,
)] = &[
    // (id, name, tier, context_window, max_output, input_cost_per_mtok, output_cost_per_mtok, supports_thinking, default_thinking_budget)
    (
        "claude-3-7-sonnet-20250219",
        "Claude 3.7 Sonnet",
        ComplexityTier::Standard,
        200_000,
        128_000, // Total limit, thinking + output
        3.0,
        15.0,
        true,
        Some(16_000),
    ),
    (
        "claude-3-5-sonnet-20241022",
        "Claude 3.5 Sonnet",
        ComplexityTier::Standard,
        200_000,
        8_192,
        3.0,
        15.0,
        false,
        None,
    ),
    (
        "claude-3-5-haiku-20241022",
        "Claude 3.5 Haiku",
        ComplexityTier::Simple,
        200_000,
        8_192,
        0.8,
        4.0,
        false,
        None,
    ),
    (
        "claude-3-opus-20240229",
        "Claude 3 Opus",
        ComplexityTier::Complex,
        200_000,
        4_096,
        15.0,
        75.0,
        false,
        None,
    ),
];

// ============================================================================
// Anthropic Provider
// ============================================================================

/// Anthropic (Claude) provider implementation.
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    models: HashMap<String, ModelInfo>,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given API key.
    pub fn new(api_key: String) -> Self {
        Self::with_options(api_key, ProviderOptions::default())
    }

    /// Create a new Anthropic provider with options.
    pub fn with_options(api_key: String, options: ProviderOptions) -> Self {
        let mut base_url = options
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());

        // Match the TS SDK behavior: if a custom base URL doesn't end with /v1, append it.
        // Some proxies like z.ai expect /v1/messages but users might only specify the root.
        base_url = base_url.trim_end_matches('/').to_string();
        if !base_url.ends_with("/v1") {
            base_url = format!("{}/v1", base_url);
        }

        let timeout_ms = options.timeout_ms.unwrap_or(300_000);
        let client = Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Build model registry
        let mut models = HashMap::new();
        for &(
            id,
            name,
            tier,
            context_window,
            max_output,
            input_cost,
            output_cost,
            supports_thinking,
            default_thinking_budget,
        ) in ANTHROPIC_MODELS
        {
            models.insert(
                id.to_string(),
                ModelInfo {
                    id: id.to_string(),
                    name: name.to_string(),
                    provider: "anthropic".to_string(),
                    tier,
                    context_window,
                    max_output_tokens: max_output,
                    supports_tool_use: true,
                    supports_vision: true,
                    supports_streaming: true,
                    supports_thinking,
                    default_thinking_budget,
                    cost_per_input_mtok: Some(input_cost),
                    cost_per_output_mtok: Some(output_cost),
                },
            );
        }

        Self {
            client,
            api_key,
            base_url,
            models,
        }
    }

    /// Build the request body for the Anthropic API.
    fn build_request_body(&self, request: &MessagesRequest) -> AnthropicRequest {
        let model_info = self.model_info(&request.model);

        let max_tokens = request.max_tokens.unwrap_or_else(|| {
            model_info
                .map(|m| m.max_output_tokens as u32)
                .unwrap_or(4096)
        });

        // Convert messages to Anthropic format
        let mut messages: Vec<AnthropicMessage> =
            request.messages.iter().map(convert_message).collect();

        // Anthropic prompt caching: only when enabled in the request.
        let use_caching = request.prompt_caching;

        if use_caching {
            // Set the cache breakpoint on the most recent user message's last block
            let mut last_user_idx = None;
            for (i, msg) in messages.iter().enumerate().rev() {
                if msg.role == "user" {
                    last_user_idx = Some(i);
                    break;
                }
            }
            if let Some(idx) = last_user_idx {
                if let Some(content_block) = messages[idx].content.last_mut() {
                    match content_block {
                        AnthropicContentBlock::Text {
                            ref mut cache_control,
                            ..
                        } => {
                            *cache_control = Some(CacheControl {
                                cache_type: "ephemeral".to_string(),
                            });
                        }
                        AnthropicContentBlock::ToolResult {
                            ref mut cache_control,
                            ..
                        } => {
                            *cache_control = Some(CacheControl {
                                cache_type: "ephemeral".to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        // Build system prompt with cache control when caching is enabled
        let system = request.system_prompt.as_ref().map(|prompt| {
            vec![AnthropicSystemBlock {
                block_type: "text".to_string(),
                text: prompt.clone(),
                cache_control: if use_caching {
                    Some(CacheControl {
                        cache_type: "ephemeral".to_string(),
                    })
                } else {
                    None
                },
            }]
        });

        // Convert tools
        let tools = if request.tools.is_empty() {
            None
        } else {
            let mut t: Vec<AnthropicTool> = request.tools.iter().map(convert_tool).collect();
            // Cache the tools array when caching is enabled
            if use_caching {
                if let Some(tool) = t.last_mut() {
                    tool.cache_control = Some(CacheControl {
                        cache_type: "ephemeral".to_string(),
                    });
                }
            }
            Some(t)
        };

        // Use thinking config if provided
        let thinking = if let Some(t) = &request.thinking {
            if t.enabled {
                Some(AnthropicThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: t.budget_tokens.unwrap_or(16000),
                })
            } else {
                None
            }
        } else {
            None
        };

        // If thinking is enabled, temperature must be 1.0 or None
        let temperature = if thinking.is_some() {
            None
        } else {
            request.temperature
        };

        AnthropicRequest {
            model: request.model.clone(),
            max_tokens,
            system,
            messages,
            tools,
            stream: true,
            temperature,
            thinking,
        }
    }

    /// Convert HTTP error to ProviderError based on status code.
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
            529 => ProviderError::Unavailable(format!("Provider overloaded: {}", body)),
            500 | 502 | 503 => ProviderError::Unavailable(format!("Server error: {}", body)),
            _ => ProviderError::StreamError(format!("HTTP {}: {}", status, body)),
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn send(
        &self,
        request: MessagesRequest,
    ) -> Result<super::traits::StreamResult, ProviderError> {
        let body = self.build_request_body(&request);

        debug!(
            model = %request.model,
            message_count = request.messages.len(),
            tool_count = request.tools.len(),
            "Sending Anthropic request"
        );

        // Make the HTTP request
        let response = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let headers = response.headers().clone();
        debug!(status = %status, "Received Anthropic response");
        tracing::trace!("Response headers: {:?}", headers);

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!(status = %status, body = %error_body, "Anthropic request failed");
            return Err(self.classify_error(status.as_u16(), &error_body));
        }

        // Create parser and translator wrapped in Arc<Mutex> for thread-safe sharing
        let parser = Arc::new(Mutex::new(SseParser::new()));
        let translator = Arc::new(Mutex::new(streaming::EventTranslator::new()));

        // Return a stream that parses SSE events
        let stream = response.bytes_stream().flat_map(move |chunk_result| {
            match chunk_result {
                Ok(bytes) => {
                    let parser_clone = parser.clone();
                    let translator_clone = translator.clone();

                    // Parse SSE events from the chunk
                    let sse_events = {
                        let mut parser = parser_clone.lock().unwrap_or_else(|e| e.into_inner());
                        parser.parse(&bytes)
                    };

                    // Translate each SSE event to StreamEvents
                    let stream_events: Vec<Result<StreamEvent, ProviderError>> = sse_events
                        .into_iter()
                        .flat_map(|sse_event| {
                            // Parse the SSE data into an AnthropicStreamEvent
                            if let Some(anthropic_event) =
                                parse_anthropic_event(&sse_event.event_type, &sse_event.data)
                            {
                                // Translate to unified StreamEvents
                                let mut translator =
                                    translator_clone.lock().unwrap_or_else(|e| e.into_inner());
                                translator.translate(anthropic_event)
                            } else {
                                Vec::new()
                            }
                        })
                        .map(Ok)
                        .collect();

                    futures::stream::iter(stream_events)
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
            }
        });

        Ok(Box::pin(stream))
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn supports_prompt_caching(&self) -> bool {
        true
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.models.values().cloned().collect()
    }

    fn model_info(&self, model_id: &str) -> Option<ModelInfo> {
        self.models.get(model_id).cloned().or_else(|| {
            // Return sensible defaults for unknown models (e.g., new releases)
            debug!(model = %model_id, "Unknown model, using default metadata");
            Some(ModelInfo {
                id: model_id.to_string(),
                name: model_id.to_string(),
                provider: "anthropic".to_string(),
                tier: ComplexityTier::Standard,
                context_window: 200_000,
                max_output_tokens: 8_192,
                supports_tool_use: true,
                supports_vision: true,
                supports_streaming: true,
                supports_thinking: false,
                default_thinking_budget: None,
                cost_per_input_mtok: Some(3.0),
                cost_per_output_mtok: Some(15.0),
            })
        })
    }

    fn count_tokens(&self, text: &str, _model: Option<&str>) -> usize {
        // Claude models use ~3.5 chars per token
        (text.len() as f64 / 3.5).ceil() as usize
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn estimate_cost(&self, model: &str, usage: &TokenUsage) -> Option<CostEstimate> {
        let info = self.model_info(model)?;

        let input_cost =
            (usage.input_tokens as f64 / 1_000_000.0) * info.cost_per_input_mtok.unwrap_or(0.0);

        let output_cost =
            (usage.output_tokens as f64 / 1_000_000.0) * info.cost_per_output_mtok.unwrap_or(0.0);

        // Adjust for cached tokens (90% discount)
        let cache_discount = if let Some(cached) = usage.cache_read_tokens {
            (cached as f64 / 1_000_000.0) * info.cost_per_input_mtok.unwrap_or(0.0) * 0.9
        } else {
            0.0
        };

        Some(CostEstimate {
            input_cost: input_cost - cache_discount,
            output_cost,
            total_cost: input_cost - cache_discount + output_cost,
            currency: "USD".to_string(),
        })
    }
}

// ============================================================================
// Message Conversion
// ============================================================================

/// Convert a unified Message to Anthropic's format.
fn convert_message(msg: &crate::providers::Message) -> AnthropicMessage {
    use crate::providers::MessageContent;

    let content = match &msg.content {
        MessageContent::Text(text) => vec![AnthropicContentBlock::Text {
            text: text.clone(),
            cache_control: None,
        }],
        MessageContent::Blocks(blocks) => blocks.iter().map(convert_block).collect(),
    };

    AnthropicMessage {
        role: match msg.role {
            crate::providers::Role::User => "user".to_string(),
            crate::providers::Role::Assistant => "assistant".to_string(),
        },
        content,
    }
}

/// Convert a unified ContentBlock to Anthropic's format.
fn convert_block(block: &crate::providers::ContentBlock) -> AnthropicContentBlock {
    use crate::providers::ContentBlock;

    match block {
        ContentBlock::Text { text } => AnthropicContentBlock::Text {
            text: text.clone(),
            cache_control: None,
        },
        ContentBlock::Thinking { thinking } => AnthropicContentBlock::Text {
            text: format!("<thinking>{}</thinking>", thinking),
            cache_control: None,
        },
        ContentBlock::ToolUse { id, name, input } => AnthropicContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => AnthropicContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
            cache_control: None,
        },
        ContentBlock::Image { source } => AnthropicContentBlock::Image {
            source: ImageSource {
                source_type: source.source_type.clone(),
                media_type: source.media_type.clone(),
                data: source.data.clone(),
            },
        },
    }
}

/// Convert a unified ToolDefinition to Anthropic's format.
fn convert_tool(tool: &crate::providers::ToolDefinition) -> AnthropicTool {
    AnthropicTool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
        cache_control: None,
    }
}

#[cfg(test)]
mod tests;
