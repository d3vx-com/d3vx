//! Conversation tests

use super::*;
use crate::providers::{ContentBlock, Message, MessageContent, Role};

#[test]
fn test_new_conversation() {
    let conv = Conversation::new();
    assert!(conv.is_empty());
    assert_eq!(conv.len(), 0);
    assert_eq!(conv.total_tokens(), 0);
}

#[test]
fn test_add_user_text() {
    let mut conv = Conversation::new();
    conv.add_user_text("Hello, world!");

    assert_eq!(conv.len(), 1);
    assert!(!conv.is_empty());

    let messages = conv.get_messages();
    assert_eq!(messages[0].role, Role::User);
}

#[test]
fn test_add_assistant_text() {
    let mut conv = Conversation::new();
    conv.add_assistant_text("Hi there!");

    assert_eq!(conv.len(), 1);

    let messages = conv.get_messages();
    assert_eq!(messages[0].role, Role::Assistant);
}

#[test]
fn test_add_multiple_messages() {
    let mut conv = Conversation::new();
    conv.add_user_text("1");
    conv.add_assistant_text("2");
    conv.add_user_text("3");

    assert_eq!(conv.len(), 3);
    assert_eq!(conv.get_messages().len(), 3);

    let messages = conv.get_messages();
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[2].role, Role::User);
}

#[test]
fn test_max_messages_pruning() {
    let mut conv = Conversation::with_max_messages(2);
    conv.add_user_text("1");
    conv.add_assistant_text("2");
    conv.add_user_text("3");

    assert_eq!(conv.len(), 2);
    let messages = conv.get_messages();
    assert_eq!(messages[0].content.as_text().unwrap(), "2");
    assert_eq!(messages[1].content.as_text().unwrap(), "3");
}

#[test]
fn test_prune_to_budget() {
    let mut conv = Conversation::new();
    conv.add_user_text("aaaa");
    conv.add_assistant_text("bbbb");
    conv.add_user_text("cccc");
    conv.add_assistant_text("dddd");

    conv.prune_to_budget(2);

    let messages = conv.get_messages();
    assert!(messages.len() >= 2);
    assert_eq!(messages[0].as_text().unwrap(), "aaaa");
    assert!(messages
        .iter()
        .any(|m| m.as_text().unwrap().contains("pruned")));
    assert_eq!(messages.last().unwrap().as_text().unwrap(), "dddd");
}

#[test]
fn test_prune_with_80_20() {
    let mut conv = Conversation::new();
    let long_text = "a".repeat(400);
    conv.add_user_text(long_text);
    conv.add_user_text("hi");

    conv.prune_to_budget(40);

    assert_eq!(conv.len(), 2);
    let messages = conv.get_messages();
    let text = messages[0].as_text().unwrap();
    assert!(text.contains("truncated"));
    assert!(conv.total_tokens() <= 40);
}

#[test]
fn test_compact() {
    let mut conv = Conversation::new();
    conv.add_user_text("first");
    conv.add_assistant_text("middle1");
    conv.add_user_text("middle2");
    conv.add_assistant_text("last");

    let removed = conv.compact(1);

    assert_eq!(removed, 2);
    assert_eq!(conv.len(), 2);
    let messages = conv.get_messages();
    assert_eq!(messages[0].content.as_text().unwrap(), "first");
    assert_eq!(messages[1].content.as_text().unwrap(), "last");
}

#[test]
fn test_clear() {
    let mut conv = Conversation::new();
    conv.add_user_text("Hello");
    conv.add_assistant_text("Hi");

    assert_eq!(conv.len(), 2);
    conv.clear();
    assert!(conv.is_empty());
    assert_eq!(conv.total_tokens(), 0);
}

#[test]
fn test_last_message() {
    let mut conv = Conversation::new();
    assert!(conv.last().is_none());

    conv.add_user_text("First");
    conv.add_assistant_text("Last");

    let last = conv.last().unwrap();
    assert_eq!(last.role, Role::Assistant);
}

#[test]
fn test_last_with_role() {
    let mut conv = Conversation::new();
    conv.add_user_text("User 1");
    conv.add_assistant_text("Assistant 1");
    conv.add_user_text("User 2");

    let last_user = conv.last_with_role(Role::User).unwrap();
    assert_eq!(last_user.as_text(), Some("User 2"));

    let last_assistant = conv.last_with_role(Role::Assistant).unwrap();
    assert_eq!(last_assistant.as_text(), Some("Assistant 1"));
}

#[test]
fn test_pop() {
    let mut conv = Conversation::new();
    conv.add_user_text("First");
    conv.add_assistant_text("Second");

    let popped = conv.pop();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().role, Role::Assistant);
    assert_eq!(conv.len(), 1);
}

#[test]
fn test_truncate() {
    let mut conv = Conversation::new();
    for i in 0..10 {
        conv.add_user_text(format!("Message {}", i));
    }

    assert_eq!(conv.len(), 10);
    conv.truncate(5);
    assert_eq!(conv.len(), 5);
    let messages = conv.get_messages();
    assert_eq!(messages[0].as_text(), Some("Message 5"));
}

#[test]
fn test_add_content_blocks() {
    let mut conv = Conversation::new();
    let blocks = vec![
        ContentBlock::text("Hello"),
        ContentBlock::tool_use("tool_1", "test_tool", serde_json::json!({"arg": "value"})),
    ];
    conv.add_user_blocks(blocks);
    assert_eq!(conv.len(), 1);
    let messages = conv.get_messages();
    assert!(matches!(messages[0].content, MessageContent::Blocks(_)));
}

#[test]
fn test_set_messages() {
    let mut conv = Conversation::new();
    let messages = vec![
        Message::user_text("First"),
        Message::assistant_text("Second"),
        Message::user_text("Third"),
    ];
    conv.set_messages(messages);
    assert_eq!(conv.len(), 3);
}

#[test]
fn test_token_estimation() {
    let mut conv = Conversation::new();
    conv.add_user_text("This is a test message.");
    assert!(conv.total_tokens() > 0);
    assert!(conv.total_tokens() < 20);
}

#[test]
fn test_compact_drain_returns_removed_messages() {
    let mut conv = Conversation::new();
    for i in 0..10 {
        conv.add_user_text(format!("Message {i}"));
    }
    assert_eq!(conv.len(), 10);

    let drained = conv.compact_drain(3);
    // Kept: first + 3 recent = 4
    assert_eq!(conv.len(), 4);
    // Drained: 10 - 4 = 6
    assert_eq!(drained.len(), 6);
    // First message preserved
    assert_eq!(conv.get_messages()[0].as_text().unwrap(), "Message 0");
    // Last message preserved
    assert_eq!(
        conv.get_messages().last().unwrap().as_text().unwrap(),
        "Message 9"
    );
}

#[test]
fn test_compact_drain_preserves_token_count() {
    let mut conv = Conversation::new();
    for i in 0..8 {
        conv.add_user_text(format!("Test message number {i}"));
    }
    let tokens_before = conv.total_tokens();
    let drained = conv.compact_drain(2);
    assert!(!drained.is_empty());
    // Token count should be recalculated for remaining messages only
    assert!(conv.total_tokens() < tokens_before);
}

#[test]
fn test_compact_drain_empty_when_few_messages() {
    let mut conv = Conversation::new();
    conv.add_user_text("One");
    conv.add_user_text("Two");
    let drained = conv.compact_drain(6);
    assert!(drained.is_empty());
    assert_eq!(conv.len(), 2);
}

#[test]
fn test_insert_after_first() {
    let mut conv = Conversation::new();
    conv.add_user_text("System context");
    conv.add_user_text("Existing message");
    assert_eq!(conv.len(), 2);

    conv.insert_after_first(Message::user_text("Inserted summary"));
    let msgs = conv.get_messages();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].as_text().unwrap(), "System context");
    assert_eq!(msgs[1].as_text().unwrap(), "Inserted summary");
    assert_eq!(msgs[2].as_text().unwrap(), "Existing message");
}

#[test]
fn test_insert_after_first_single_message() {
    let mut conv = Conversation::new();
    conv.add_user_text("Only message");
    conv.insert_after_first(Message::user_text("Appended"));
    let msgs = conv.get_messages();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].as_text().unwrap(), "Only message");
    assert_eq!(msgs[1].as_text().unwrap(), "Appended");
}
