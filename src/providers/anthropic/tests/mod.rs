// Tests for Anthropic provider
use super::{convert_block, AnthropicContentBlock, AnthropicProvider};
use crate::providers::{
    ContentBlock, ImageSource as UnifiedImageSource, Message, MessageContent, MessagesRequest,
    Provider, ProviderError, Role, TokenUsage, ToolSchema,
};
use std::collections::HashMap;

#[test]
fn test_provider_creation() {
    let provider = AnthropicProvider::new("test-key".to_string());
    assert_eq!(provider.name(), "anthropic");
    assert!(provider.is_available());
}

#[test]
fn test_models_registered() {
    let provider = AnthropicProvider::new("test-key".to_string());
    let models = provider.models();
    assert!(!models.is_empty());

    // Check that Sonnet 4 is registered
    assert!(provider.model_info("claude-3-7-sonnet-20250219").is_some());
    assert!(
        provider
            .model_info("claude-3-7-sonnet-20250219")
            .unwrap()
            .supports_thinking
    );
}

#[test]
fn test_token_counting() {
    let provider = AnthropicProvider::new("test-key".to_string());

    // Test approximate token counting
    let text = "Hello, world! This is a test.";
    let tokens = provider.count_tokens(text, None);

    // Should be approximately text.len() / 3.5
    assert!(tokens > 0);
    assert!(tokens < text.len());
}

#[test]
fn test_unknown_model_defaults() {
    let provider = AnthropicProvider::new("test-key".to_string());

    // Unknown model should return sensible defaults
    let info = provider.model_info("claude-future-model-2026").unwrap();
    assert_eq!(info.id, "claude-future-model-2026");
    assert_eq!(info.context_window, 200_000);
    assert!(info.supports_tool_use);
}

#[test]
fn test_cost_estimation() {
    let provider = AnthropicProvider::new("test-key".to_string());

    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 500_000,
        reasoning_tokens: 0,
        cache_read_tokens: None,
        cache_write_tokens: None,
    };

    let cost = provider
        .estimate_cost("claude-3-7-sonnet-20250219", &usage)
        .unwrap();

    // Input: 1M tokens at $3/MTok = $3
    // Output: 500K tokens at $15/MTok = $7.50
    assert!((cost.input_cost - 3.0).abs() < 0.01);
    assert!((cost.output_cost - 7.5).abs() < 0.01);
    assert!((cost.total_cost - 10.5).abs() < 0.01);
}

#[test]
fn test_error_classification() {
    let provider = AnthropicProvider::new("test-key".to_string());

    // 401 -> AuthFailed
    let err = provider.classify_error(401, "Invalid API key");
    assert!(matches!(err, ProviderError::AuthFailed(_)));

    // 429 -> RateLimited
    let err = provider.classify_error(429, "Too many requests");
    assert!(matches!(err, ProviderError::RateLimited(_)));

    // 400 with 'context' -> ContextExceeded
    let err = provider.classify_error(400, "prompt is too long for the context window");
    assert!(matches!(err, ProviderError::ContextExceeded(_)));

    // 500 -> Unavailable
    let err = provider.classify_error(500, "Internal Server Error");
    assert!(matches!(err, ProviderError::Unavailable(_)));
}

#[test]
fn test_message_conversion_with_cache() {
    let provider = AnthropicProvider::new("test-key".to_string());
    let request = MessagesRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        }],
        system_prompt: Some("You are a helper".to_string()),
        tools: vec![crate::providers::ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: ToolSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: None,
            },
        }],
        ..Default::default()
    };

    let body = provider.build_request_body(&request);

    // Verify system prompt caching
    assert!(body.system.is_some());
    assert!(body.system.unwrap()[0].cache_control.is_some());

    // Verify message caching (last user message)
    assert!(body.messages[0].content[0]
        .clone()
        .cache_control_type()
        .is_some());

    // Verify tool caching (last tool)
    assert!(body.tools.unwrap()[0].cache_control.is_some());
}

#[test]
fn test_convert_complex_blocks() {
    // Test Image conversion
    let unified_block = ContentBlock::Image {
        source: UnifiedImageSource {
            source_type: "base64".to_string(),
            media_type: "image/png".to_string(),
            data: "abc".to_string(),
        },
    };
    let anthropic_block = convert_block(&unified_block);
    if let AnthropicContentBlock::Image { source } = anthropic_block {
        assert_eq!(source.media_type, "image/png");
        assert_eq!(source.data, "abc");
    } else {
        panic!("Expected Image block");
    }

    // Test ToolResult conversion
    let unified_block = ContentBlock::ToolResult {
        tool_use_id: "id123".to_string(),
        content: "success".to_string(),
        is_error: Some(false),
    };
    let anthropic_block = convert_block(&unified_block);
    if let AnthropicContentBlock::ToolResult {
        tool_use_id,
        content,
        is_error,
        ..
    } = anthropic_block
    {
        assert_eq!(tool_use_id, "id123");
        assert_eq!(content, "success");
        assert_eq!(is_error, Some(false));
    } else {
        panic!("Expected ToolResult block");
    }
}

/// Helper extension to check cache control in tests
trait CacheCheck {
    fn cache_control_type(&self) -> Option<String>;
}

impl CacheCheck for AnthropicContentBlock {
    fn cache_control_type(&self) -> Option<String> {
        match self {
            AnthropicContentBlock::Text { cache_control, .. } => {
                cache_control.as_ref().map(|c| c.cache_type.clone())
            }
            AnthropicContentBlock::ToolResult { cache_control, .. } => {
                cache_control.as_ref().map(|c| c.cache_type.clone())
            }
            _ => None,
        }
    }
}
