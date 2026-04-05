//! Core provider types
//!
//! Common types shared across all LLM providers: messages, content blocks,
//! tool definitions, stream events, model metadata, and configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use super::traits::ProviderError;

// ============================================================================
// Message Types
// ============================================================================

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

impl Message {
    /// Create a new user message with text content.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a new assistant message with text content.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a new user message with content blocks.
    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Create a new assistant message with content blocks.
    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Get text content if this is a simple text message or the first text block.
    pub fn as_text(&self) -> Option<&str> {
        self.content.as_text()
    }

    /// Get content blocks if this is structured content.
    pub fn as_blocks(&self) -> Option<&[ContentBlock]> {
        match &self.content {
            MessageContent::Text(_) => None,
            MessageContent::Blocks(blocks) => Some(blocks),
        }
    }
}

impl MessageContent {
    /// Get text content if this is a simple text message or the first text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Blocks(blocks) => blocks.iter().find_map(|b| b.as_text()),
        }
    }
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Message content - either a simple string or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Image {
        source: ImageSource,
    },
}

impl ContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a thinking content block.
    pub fn thinking(thinking: impl Into<String>) -> Self {
        Self::Thinking {
            thinking: thinking.into(),
        }
    }

    /// Create a tool use content block.
    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a tool result content block.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: None,
        }
    }

    /// Create a tool error result content block.
    pub fn tool_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: Some(true),
        }
    }

    /// Check if this is a text block.
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    /// Check if this is a tool use block.
    pub fn is_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    /// Check if this is a tool result block.
    pub fn is_tool_result(&self) -> bool {
        matches!(self, Self::ToolResult { .. })
    }

    /// Get text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// Image source for vision capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String, // "image/jpeg", "image/png", etc.
    pub data: String,       // Base64-encoded image data
}

// ============================================================================
// Tool Types
// ============================================================================

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolSchema,
}

/// JSON Schema for tool inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String, // "object"
    pub properties: HashMap<String, ToolParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

/// A parameter in a tool schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ToolParameter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

// ============================================================================
// Request Types
// ============================================================================

/// Request to send messages to an LLM.
#[derive(Debug, Clone)]
pub struct MessagesRequest {
    /// Model ID to use.
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// System prompt (optional).
    pub system_prompt: Option<String>,
    /// Available tools (optional).
    pub tools: Vec<ToolDefinition>,
    /// Maximum output tokens.
    pub max_tokens: Option<u32>,
    /// Temperature for sampling.
    pub temperature: Option<f32>,
    /// Thinking configuration (optional).
    pub thinking: Option<ThinkingConfig>,
    /// Whether to enable prompt caching for providers that support it.
    /// Ignored by providers that don't support cache_control.
    pub prompt_caching: bool,
}

impl Default for MessagesRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            max_tokens: None,
            temperature: None,
            thinking: None,
            prompt_caching: true,
        }
    }
}

/// Thinking configuration for the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Enable thinking mode.
    pub enabled: bool,
    /// Internal token budget for thinking (mostly for Anthropic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    /// Reasoning effort level (mostly for OpenAI).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Reasoning effort levels for models that support it (e.g., OpenAI o1/o3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

// ============================================================================
// Stream Events
// ============================================================================

/// Events emitted during streaming response.
#[derive(Debug)]
pub enum StreamEvent {
    /// Message started.
    MessageStart {
        id: String,
        model: String,
        usage: TokenUsage,
    },
    /// Text content delta.
    TextDelta { text: String },
    /// Thinking content delta (extended thinking).
    ThinkingDelta { text: String },
    /// Tool use block started.
    ToolUseStart { id: String, name: String },
    /// Tool use input JSON delta.
    ToolUseDelta { input_json: String },
    /// Tool use block completed.
    ToolUseEnd {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Message completed.
    MessageEnd {
        usage: TokenUsage,
        stop_reason: StopReason,
    },
    /// Error occurred.
    Error { error: ProviderError },
}

/// Token usage statistics (OpenCode parity).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.reasoning_tokens
    }

    pub fn adjusted_input(&self) -> u64 {
        self.input_tokens.saturating_sub(
            self.cache_read_tokens.unwrap_or(0) + self.cache_write_tokens.unwrap_or(0),
        )
    }
}

/// Reason for message completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

/// Complexity tier of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityTier {
    /// Fast/cheap models (haiku, flash, etc.)
    Simple,
    /// Balanced models (sonnet, gpt-4o, etc.)
    Standard,
    /// Powerful reasoning models (opus, o1, etc.)
    Complex,
}

impl std::fmt::Display for ComplexityTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Simple => "simple",
            Self::Standard => "standard",
            Self::Complex => "complex",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Model Metadata
// ============================================================================

/// Metadata about an LLM model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub tier: ComplexityTier,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub supports_tool_use: bool,
    pub supports_vision: bool,
    pub supports_streaming: bool,
    pub supports_thinking: bool,
    pub default_thinking_budget: Option<u32>,
    pub cost_per_input_mtok: Option<f64>,
    pub cost_per_output_mtok: Option<f64>,
}

// ============================================================================
// Configuration
// ============================================================================

/// Provider configuration options.
#[derive(Debug, Clone, Default)]
pub struct ProviderOptions {
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u32>,
}
