use super::types::MessageList;
use crate::ipc::{Message, MessageRole};
use chrono::Utc;

fn make_test_message(role: MessageRole, content: &str) -> Message {
    Message {
        id: uuid::Uuid::new_v4().to_string(),
        role,
        content: content.to_string(),
        timestamp: Utc::now(),
        is_error: false,
        tool_calls: Vec::new(),
        is_streaming: false,
        shell_cmd: None,
        exit_code: None,
    }
}

#[test]
fn test_message_list_renders_user_message() {
    let msg = make_test_message(MessageRole::User, "Hello, world!");
    let messages = vec![msg];
    let list = MessageList::new(&messages);
    let lines = list.build_lines();

    assert!(lines
        .iter()
        .any(|l| l.spans.iter().any(|s| s.content.contains("You"))));
}

#[test]
fn test_message_list_renders_assistant_message() {
    let msg = make_test_message(MessageRole::Assistant, "Hello back!");
    let messages = vec![msg];
    let list = MessageList::new(&messages);
    let lines = list.build_lines();

    assert!(lines
        .iter()
        .any(|l| l.spans.iter().any(|s| s.content.contains("d3vx"))));
}
