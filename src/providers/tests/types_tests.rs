//! Tests for provider core types (TokenUsage, Message, ContentBlock, Role, etc.)

use crate::providers::{
    ComplexityTier, Message, MessagesRequest, ReasoningEffort,
    Role, ThinkingConfig,
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

// ── ComplexityTier Tests ──────────────────────────────────────

#[test]
fn test_complexity_tier_display() {
    assert_eq!(ComplexityTier::Simple.to_string(), "simple");
    assert_eq!(ComplexityTier::Standard.to_string(), "standard");
    assert_eq!(ComplexityTier::Complex.to_string(), "complex");
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
    assert!(json.contains("enabled"));
}

// ── MessagesRequest Tests ─────────────────────────────────────

#[test]
fn test_messages_request_default() {
    let req = MessagesRequest::default();
    assert_eq!(req.model, "");
    assert!(req.messages.is_empty());
}
