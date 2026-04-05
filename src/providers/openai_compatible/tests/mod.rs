// Tests for OpenAI Compatible provider
use super::{
    convert_message, groq_config, ollama_config, openai_config, openrouter_config,
    OpenAICompatibleProvider,
};
use crate::providers::{
    ContentBlock, Message, MessageContent, MessagesRequest, Provider, ProviderError, Role,
    TokenUsage, ToolDefinition, ToolSchema,
};
use std::collections::HashMap;

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

#[test]
fn test_error_classification() {
    let config = openai_config("test-key".to_string(), None);
    let provider = OpenAICompatibleProvider::new(config);

    // 401 -> AuthFailed
    let err = provider.classify_error(401, "Invalid API key");
    assert!(matches!(err, ProviderError::AuthFailed(_)));

    // 429 -> RateLimited
    let err = provider.classify_error(429, "Too many requests");
    assert!(matches!(err, ProviderError::RateLimited(_)));

    // 500 -> Unavailable
    let err = provider.classify_error(500, "Internal Server Error");
    assert!(matches!(err, ProviderError::Unavailable(_)));
}

#[test]
fn test_message_conversion_complex() {
    // Test Text conversion
    let msg = Message {
        role: Role::User,
        content: MessageContent::Text("Hello".into()),
    };
    let converted = convert_message(&msg);
    assert_eq!(converted["role"], "user");
    assert_eq!(converted["content"], "Hello");

    // Test ToolUse conversion
    let msg = Message {
        role: Role::Assistant,
        content: MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Thinking...".into(),
            },
            ContentBlock::ToolUse {
                id: "call123".into(),
                name: "my_tool".into(),
                input: serde_json::json!({"arg": 1}),
            },
        ]),
    };
    let converted = convert_message(&msg);
    assert_eq!(converted["role"], "assistant");
    assert_eq!(converted["tool_calls"][0]["id"], "call123");
    assert_eq!(converted["tool_calls"][0]["function"]["name"], "my_tool");
    assert_eq!(converted["content"], "Thinking...");

    // Test ToolResult conversion
    let msg = Message {
        role: Role::User,
        content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "call123".into(),
            content: "result text".into(),
            is_error: Some(false),
        }]),
    };
    let converted = convert_message(&msg);
    assert_eq!(converted["role"], "tool");
    assert_eq!(converted["tool_call_id"], "call123");
    assert_eq!(converted["content"], "result text");
}

#[test]
fn test_request_body_generation() {
    let config = openai_config("test-key".to_string(), None);
    let provider = OpenAICompatibleProvider::new(config);
    let request = MessagesRequest {
        model: "gpt-4o".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hi".into()),
        }],
        tools: vec![ToolDefinition {
            name: "t1".into(),
            description: "d1".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties: HashMap::new(),
                required: None,
            },
        }],
        ..Default::default()
    };

    let body = provider.build_request_body(&request);
    assert_eq!(body["model"], "gpt-4o");
    assert!(body["tools"].is_array());
    assert_eq!(body["tools"][0]["function"]["name"], "t1");
}
