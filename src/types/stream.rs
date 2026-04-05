//! Stream event types for unified LLM provider communication
//!
//! Stream events are normalized across all providers so the TUI
//! can handle them uniformly regardless of the underlying LLM service.

use serde::{Deserialize, Serialize};

/// Token usage statistics from LLM responses (OpenCode parity).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of input tokens
    pub input_tokens: u64,
    /// Number of output tokens
    pub output_tokens: u64,
    /// Number of reasoning/thinking tokens (for o1/o3 models)
    pub reasoning_tokens: u64,
    /// Number of tokens read from cache (if supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    /// Number of tokens written to cache (if supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
}

impl TokenUsage {
    /// Create new token usage stats
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            reasoning_tokens: 0,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    /// Total tokens used (input + output + reasoning)
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.reasoning_tokens
    }

    /// Adjusted input tokens (excludes cached tokens)
    pub fn adjusted_input(&self) -> u64 {
        self.input_tokens.saturating_sub(
            self.cache_read_tokens.unwrap_or(0) + self.cache_write_tokens.unwrap_or(0),
        )
    }
}

/// Stop reason for message completion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Normal completion
    EndTurn,
    /// Tool use requested
    ToolUse,
    /// Max tokens limit reached
    MaxTokens,
    /// Stop sequence encountered
    StopSequence,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::EndTurn => write!(f, "end_turn"),
            StopReason::ToolUse => write!(f, "tool_use"),
            StopReason::MaxTokens => write!(f, "max_tokens"),
            StopReason::StopSequence => write!(f, "stop_sequence"),
        }
    }
}

/// Cost estimate for token usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Cost for input tokens
    pub input_cost: f64,
    /// Cost for output tokens
    pub output_cost: f64,
    /// Total cost
    pub total_cost: f64,
    /// Currency (usually "USD")
    pub currency: String,
}

impl CostEstimate {
    /// Create a new cost estimate in USD
    pub fn usd(input_cost: f64, output_cost: f64) -> Self {
        Self {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            currency: "USD".to_string(),
        }
    }
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "claude-3-opus-20240229")
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,
    /// Context window size
    pub context_window: u64,
    /// Maximum output tokens
    pub max_output_tokens: u64,
    /// Whether the model supports tool use
    pub supports_tool_use: bool,
    /// Whether the model supports vision/images
    pub supports_vision: bool,
    /// Whether the model supports streaming
    pub supports_streaming: bool,
    /// Whether the model supports extended thinking
    #[serde(default)]
    pub supports_thinking: bool,
    /// Cost per million input tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_input_mtok: Option<f64>,
    /// Cost per million output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_output_mtok: Option<f64>,
}

/// Stream event: message started
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartEvent {
    /// Message ID
    pub id: String,
    /// Model used
    pub model: String,
}

/// Stream event: text delta received
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDeltaEvent {
    /// Text content
    pub text: String,
}

/// Stream event: thinking delta received
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingDeltaEvent {
    /// Thinking content
    pub text: String,
}

/// Stream event: tool use started
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseStartEvent {
    /// Tool use ID
    pub id: String,
    /// Tool name
    pub name: String,
}

/// Stream event: tool use input delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseDeltaEvent {
    /// JSON fragment
    pub input_json: String,
}

/// Stream event: tool use completed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseEndEvent {
    /// Tool use ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Complete input as JSON
    pub input: serde_json::Value,
}

/// Stream event: message completed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEndEvent {
    /// Token usage
    pub usage: TokenUsage,
    /// Stop reason
    pub stop_reason: StopReason,
}

/// Stream event: error occurred
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    /// Error message
    pub error: String,
}

/// Unified stream events from all LLM providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Message started
    MessageStart(MessageStartEvent),
    /// Text delta received
    TextDelta(TextDeltaEvent),
    /// Thinking delta received
    ThinkingDelta(ThinkingDeltaEvent),
    /// Tool use started
    ToolUseStart(ToolUseStartEvent),
    /// Tool use input delta
    ToolUseDelta(ToolUseDeltaEvent),
    /// Tool use completed
    ToolUseEnd(ToolUseEndEvent),
    /// Message completed
    MessageEnd(MessageEndEvent),
    /// Error occurred
    Error(ErrorEvent),
}

/// Parameters for sending messages to LLM providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageParams {
    /// Model to use
    pub model: String,
    /// Messages to send
    pub messages: Vec<super::Message>,
    /// System prompt
    pub system_prompt: String,
    /// Available tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<super::ToolDefinition>>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_stop_reason_serialization() {
        let reason = StopReason::ToolUse;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#""tool_use""#);
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::TextDelta(TextDeltaEvent {
            text: "Hello".to_string(),
        });
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains(r#""type":"text_delta""#));
        assert!(json.contains(r#""text":"Hello""#));
    }

    #[test]
    fn test_message_start_event() {
        let event = StreamEvent::MessageStart(MessageStartEvent {
            id: "msg_123".to_string(),
            model: "claude-3-opus".to_string(),
        });
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains(r#""type":"message_start""#));
        assert!(json.contains(r#""id":"msg_123""#));
    }

    #[test]
    fn test_message_end_event() {
        let event = StreamEvent::MessageEnd(MessageEndEvent {
            usage: TokenUsage::new(100, 50),
            stop_reason: StopReason::EndTurn,
        });
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains(r#""type":"message_end""#));
        assert!(json.contains(r#""stop_reason":"end_turn""#));
    }

    #[test]
    fn test_cost_estimate() {
        let cost = CostEstimate::usd(0.01, 0.02);
        assert_eq!(cost.total_cost, 0.03);
        assert_eq!(cost.currency, "USD");
    }
}
