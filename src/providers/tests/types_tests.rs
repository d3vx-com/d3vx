//! Tests for provider core types (TokenUsage, Message, ContentBlock, Role, etc.)

use d3vx::providers::{
    ComplexityTier, ContentBlock, Message, MessageContent, MessagesRequest, ReasoningEffort,
    Role, StopReason, ThinkingConfig, TokenUsage, ToolDefinition,
};

// ── Role Tests ────────────────────────────────────────────────

#[test]
fn test_role_equality() {
    assert_eq!(Role::User, Role::User);
    assert_ne!(Role::User, Role::Assistant);
}

#[test]
fn test_role_serde_user() {
    let json = serde_json::to_string(&Role::User).unwrap();
    assert_eq!(json, r#""user""#);
    let parsed: Role = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, Role::User);
}

#[test]
fn test_role_serde_assistant() {
    let json = serde_json::to_string(&Role::Assistant).unwrap();
    assert_eq!(json, r#""assistant""#);
    let parsed: Role = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, Role::Assistant);
}

// ── Message Tests ─────────────────────────────────────────────

#[test]
fn test_message_user_text() {
    let msg = Message::user_text("Hello");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.as_text(), Some("Hello"));
}

#[test]
fn test_message_assistant_text() {
    let msg = Message::assistant_text("Hello back");
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.as_text(), Some("Hello back"));
}

#[test]
fn test_message_user_blocks() {
    let blocks = vec![ContentBlock::text("Hi"), ContentBlock::thinking("thinking...")];
    let msg = Message::user_blocks(blocks.clone());
    assert_eq!(msg.role, Role::User);
    let got = msg.as_blocks().unwrap();
    assert_eq!(got.len(), 2);
}

#[test]
fn test_message_as_text_from_blocks_first_text() {
    let blocks = vec![
        ContentBlock::thinking("thinking"),
        ContentBlock::text("Actual response"),
    ];
    let msg = Message::assistant_blocks(blocks);
    // as_text should find the first text block
    assert_eq!(msg.as_text(), Some("Actual response"));
}

#[test]
fn test_message_as_text_no_text_block() {
    let blocks = vec![ContentBlock::thinking("deep thought")];
    let msg = Message::assistant_blocks(blocks);
    // No text block present
    assert_eq!(msg.as_text(), None);
}

#[test]
fn test_message_as_blocks_on_text_message() {
    let msg = Message::user_text("simple");
    assert!(msg.as_blocks().is_none());
}

// ── ContentBlock Tests ────────────────────────────────────────

#[test]
fn test_content_block_text_factory() {
    let block = ContentBlock::text("hello");
    assert!(block.is_text());
    assert!(!block.is_tool_use());
    assert!(!block.is_tool_result());
    assert_eq!(block.as_text(), Some("hello"));
}

#[test]
fn test_content_block_thinking_factory() {
    let block = ContentBlock::thinking("hmm");
    assert!(!block.is_text());
    assert_eq!(block.as_text(), None);
}

#[test]
fn test_content_block_tool_use_factory() {
    let block = ContentBlock::tool_use("t1", "Bash", serde_json::json!({"cmd": "ls"}));
    assert!(!block.is_text());
    assert!(block.is_tool_use());
    assert!(!block.is_tool_result());
}

#[test]
fn test_content_block_tool_result_factory() {
    let block = ContentBlock::tool_result("t1", "output");
    assert!(block.is_tool_result());
    assert!(!block.is_tool_use());
}

#[test]
fn test_content_block_tool_error_factory() {
    let block = ContentBlock::tool_error("t1", "command failed");
    assert!(block.is_tool_result());
    // Verify is_error is set to Some(true)
    match block {
        ContentBlock::ToolResult { is_error, .. } => assert_eq!(is_error, Some(true)),
        _ => panic!("Expected ToolResult"),
    }
}

#[test]
fn test_content_block_serde_text() {
    let block = ContentBlock::text("hello");
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "text");
    assert_eq!(json["text"], "hello");
}

#[test]
fn test_content_block_serde_tool_use() {
    let block = ContentBlock::tool_use("t1", "Bash", serde_json::json!({"a": 1}));
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "tool_use");
    assert_eq!(json["id"], "t1");
    assert_eq!(json["name"], "Bash");
}

#[test]
fn test_content_block_roundtrip_thinking() {
    let block = ContentBlock::thinking("reasoning...");
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    match parsed {
        ContentBlock::Thinking { ref thinking } => assert_eq!(thinking, "reasoning..."),
        _ => panic!("Expected Thinking variant"),
    }
}

// ── TokenUsage Tests ──────────────────────────────────────────

#[test]
fn test_token_usage_default() {
    let usage = TokenUsage::default();
    assert_eq!(usage.total(), 0);
    assert_eq!(usage.adjusted_input(), 0);
}

#[test]
fn test_token_usage_total() {
    let usage = TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        reasoning_tokens: 25,
        cache_read_tokens: None,
        cache_write_tokens: None,
    };
    assert_eq!(usage.total(), 175);
}

#[test]
fn test_token_usage_adjusted_input_no_cache() {
    let usage = TokenUsage {
        input_tokens: 100,
        output_tokens: 0,
        reasoning_tokens: 0,
        cache_read_tokens: None,
        cache_write_tokens: None,
    };
    assert_eq!(usage.adjusted_input(), 100);
}

#[test]
fn test_token_usage_adjusted_input_with_cache() {
    let usage = TokenUsage {
        input_tokens: 200,
        output_tokens: 0,
        reasoning_tokens: 0,
        cache_read_tokens: Some(100),
        cache_write_tokens: Some(50),
    };
    assert_eq!(usage.adjusted_input(), 50);
}

#[test]
fn test_token_usage_adjusted_input_saturates() {
    // cache tokens exceed input — should saturate at 0
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 0,
        reasoning_tokens: 0,
        cache_read_tokens: Some(20),
        cache_write_tokens: Some(0),
    };
    assert_eq!(usage.adjusted_input(), 0);
}

// ── StopReason Tests ──────────────────────────────────────────

#[test]
fn test_stop_reason_serde() {
    let json = serde_json::to_string(&StopReason::EndTurn).unwrap();
    assert_eq!(json, r#""end_turn""#);

    let json = serde_json::to_string(&StopReason::ToolUse).unwrap();
    assert_eq!(json, r#""tool_use""#);

    let json = serde_json::to_string(&StopReason::MaxTokens).unwrap();
    assert_eq!(json, r#""max_tokens""#);

    let json = serde_json::to_string(&StopReason::StopSequence).unwrap();
    assert_eq!(json, r#""stop_sequence""#);
}

#[test]
fn test_stop_reason_roundtrip() {
    for reason in [
        StopReason::EndTurn,
        StopReason::ToolUse,
        StopReason::MaxTokens,
        StopReason::StopSequence,
    ] {
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: StopReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, reason);
    }
}

// ── ComplexityTier Tests ──────────────────────────────────────

#[test]
fn test_complexity_tier_display() {
    assert_eq!(ComplexityTier::Simple.to_string(), "simple");
    assert_eq!(ComplexityTier::Standard.to_string(), "standard");
    assert_eq!(ComplexityTier::Complex.to_string(), "complex");
}

#[test]
fn test_complexity_tier_serde() {
    let json = serde_json::to_string(&ComplexityTier::Simple).unwrap();
    assert_eq!(json, r#""simple""#);
}

// ── ReasoningEffort Tests ─────────────────────────────────────

#[test]
fn test_reasoning_effort_serde() {
    let json = serde_json::to_string(&ReasoningEffort::Low).unwrap();
    assert_eq!(json, r#""low""#);
    assert_eq!(
        serde_json::from_str::<ReasoningEffort>(&json).unwrap(),
        ReasoningEffort::Low
    );
}

// ── ThinkingConfig Tests ──────────────────────────────────────

#[test]
fn test_thinking_config_minimal() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        reasoning_effort: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    // Should only contain enabled field
    assert!(json.contains("enabled"));
}

#[test]
fn test_thinking_config_full() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(1000),
        reasoning_effort: Some(ReasoningEffort::High),
    };
    assert!(config.budget_tokens.is_some());
    assert_eq!(config.reasoning_effort, Some(ReasoningEffort::High));
}

// ── MessagesRequest Tests ─────────────────────────────────────

#[test]
fn test_messages_request_default() {
    let req = MessagesRequest::default();
    assert_eq!(req.model, "");
    assert!(req.messages.is_empty());
    assert_eq!(req.tools.len(), 0);
    assert!(req.prompt_caching);
    assert!(req.system_prompt.is_none());
    assert!(req.temperature.is_none());
}

#[test]
fn test_messages_request_with_model() {
    let req = MessagesRequest {
        model: "claude-sonnet-4".to_string(),
        ..Default::default()
    };
    assert_eq!(req.model, "claude-sonnet-4");
}
