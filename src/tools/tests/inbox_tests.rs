//! Inbox Tool Tests

use crate::tools::inbox::{ReadInboxTool, SendInboxMessageTool};
use crate::tools::types::{Tool, ToolContext};
use serde_json::json;

// -- SendInboxMessageTool tests --

#[tokio::test]
async fn send_inbox_message_missing_to_agent() {
    let tool = SendInboxMessageTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({"message": "hi"}), &ctx).await;

    assert!(r.is_error);
    assert!(r.content.contains("to_agent"));
}

#[tokio::test]
async fn send_inbox_message_missing_message() {
    let tool = SendInboxMessageTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({"to_agent": "tech_lead"}), &ctx).await;

    assert!(r.is_error);
    assert!(r.content.contains("message"));
}

#[test]
fn send_inbox_message_tool_name() {
    let tool = SendInboxMessageTool::new();
    assert_eq!(tool.name(), "send_inbox_message");
}

#[test]
fn send_inbox_message_definition_has_required_fields() {
    let tool = SendInboxMessageTool::new();
    let def = tool.definition();
    assert_eq!(def.name, "send_inbox_message");
    let req = def.input_schema["required"].as_array().unwrap();
    assert!(req.iter().any(|v| v == "to_agent"));
    assert!(req.iter().any(|v| v == "message"));
}

// -- ReadInboxTool tests --

#[tokio::test]
async fn read_inbox_when_empty_returns_success() {
    let tool = ReadInboxTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({}), &ctx).await;

    assert!(!r.is_error);
    assert!(r.content.contains("Inbox is empty"));
}

#[test]
fn read_inbox_tool_name() {
    let tool = ReadInboxTool::new();
    assert_eq!(tool.name(), "read_inbox");
}

#[test]
fn read_inbox_definition_no_required_fields() {
    let tool = ReadInboxTool::new();
    let def = tool.definition();
    assert_eq!(def.name, "read_inbox");
    assert!(def.description.contains("pending"));
}
