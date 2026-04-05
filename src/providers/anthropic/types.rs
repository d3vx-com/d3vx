//! Anthropic API Types
//!
//! Request and response types for the Anthropic Messages API.
//! These types match the Anthropic API specification.

use serde::{Deserialize, Serialize};

// ============================================================================
// Request Types
// ============================================================================

/// Request to create a message (streaming).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicRequest {
    /// Model ID (e.g., "claude-sonnet-4-20250514")
    pub model: String,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// System prompt blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<AnthropicSystemBlock>>,
    /// Conversation messages
    pub messages: Vec<AnthropicMessage>,
    /// Available tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    /// Enable streaming
    pub stream: bool,
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Thinking configuration (Extended Thinking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingConfig>,
}

/// Thinking configuration for Anthropic.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String, // "enabled"
    pub budget_tokens: u32,
}

/// System prompt block with optional cache control.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicSystemBlock {
    #[serde(rename = "type")]
    pub block_type: String, // "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl Default for AnthropicSystemBlock {
    fn default() -> Self {
        Self {
            block_type: "text".to_string(),
            text: String::new(),
            cache_control: None,
        }
    }
}

/// Cache control directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String, // "ephemeral"
}

/// Message in the conversation.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicMessage {
    pub role: String, // "user" or "assistant"
    pub content: Vec<AnthropicContentBlock>,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "input_schema")]
    pub input_schema: crate::providers::ToolSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

// ... wait I need to update AnthropicContentBlock too
/// Content block in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ImageSource,
    },
}

/// Image source for vision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String, // "image/jpeg", "image/png", etc.
    pub data: String,       // Base64-encoded
}

// ============================================================================
// Response Types (Streaming Events)
// ============================================================================

/// SSE event from Anthropic's streaming API.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: u32,
        content_block: ContentBlockStart,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: MessageDeltaUsage,
    },
    MessageStop,
    Ping,
    Error {
        error: ErrorData,
    },
}

/// Data for message_start event.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<serde_json::Value>,
    pub stop_reason: Option<String>,
    pub usage: MessageUsage,
}

/// Usage statistics in message_start.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageUsage {
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
}

/// Content block start event.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStart {
    ToolUse { id: String, name: String },
    Text { text: String },
    Thinking { thinking: String },
}

/// Content delta event.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

/// Message delta event data.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
}

/// Usage in message_delta event.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaUsage {
    pub output_tokens: u64,
}

/// Error data.
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorData {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

// ============================================================================
// Anthropic API Error Response
// ============================================================================

/// Error response from the Anthropic API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicError {
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let request = AnthropicRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 1024,
            system: None,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: vec![AnthropicContentBlock::Text {
                    text: "Hello".to_string(),
                    cache_control: None,
                }],
            }],
            tools: None,
            stream: true,
            temperature: None,
            thinking: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("claude-sonnet-4-20250514"));
        assert!(json.contains("max_tokens"));
    }

    #[test]
    fn test_deserialize_message_start() {
        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_123",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-20250514",
                "content": [],
                "stop_reason": null,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 0
                }
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_123");
                assert_eq!(message.model, "claude-sonnet-4-20250514");
            }
            _ => panic!("Expected MessageStart event"),
        }
    }

    #[test]
    fn test_deserialize_content_delta() {
        let json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": "Hello"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => {
                    assert_eq!(text, "Hello");
                }
                _ => panic!("Expected TextDelta"),
            },
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }
}
