//! Tests for IPC event handler helpers.
//!
//! Focused on `route_tool_call_update` — the tool-call routing path that
//! previously assumed updates always belonged to the newest message. In
//! multi-agent sessions that assumption is wrong: another agent can push
//! a message between a tool_call's creation and its status update.

use super::ipc::route_tool_call_update;
use crate::ipc::{Message, ToolCall, ToolStatus};

fn tool_call(id: &str, name: &str, status: ToolStatus) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        input: serde_json::Value::Null,
        status,
        output: None,
        elapsed: None,
    }
}

fn assistant_with_tool_calls(text: &str, tool_calls: Vec<ToolCall>) -> Message {
    let mut m = Message::assistant(text);
    m.tool_calls = tool_calls;
    m
}

#[test]
fn updates_tool_call_on_owning_message_when_it_is_last() {
    let initial = tool_call("tc-1", "bash", ToolStatus::Running);
    let mut messages = vec![assistant_with_tool_calls("working", vec![initial])];

    let update = tool_call("tc-1", "bash", ToolStatus::Completed);
    route_tool_call_update(&mut messages, update);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tool_calls.len(), 1);
    assert_eq!(messages[0].tool_calls[0].status, ToolStatus::Completed);
}

#[test]
fn updates_tool_call_on_earlier_message_when_another_was_pushed() {
    // This is the multi-agent race scenario. Agent A emits a tool_call on
    // message[0]; Agent B then pushes a new message[1]; Agent A's update
    // must still land on message[0], not be appended to message[1].
    let a_tool_call = tool_call("tc-A", "bash", ToolStatus::Running);
    let mut messages = vec![
        assistant_with_tool_calls("agent A working", vec![a_tool_call]),
        Message::assistant("agent B's unrelated reply"),
    ];

    let update = tool_call("tc-A", "bash", ToolStatus::Completed);
    route_tool_call_update(&mut messages, update);

    // Owner (message[0]) is updated in place.
    assert_eq!(messages[0].tool_calls.len(), 1);
    assert_eq!(messages[0].tool_calls[0].status, ToolStatus::Completed);
    // Newest message (message[1]) is untouched.
    assert!(
        messages[1].tool_calls.is_empty(),
        "update must not leak onto unrelated newer message"
    );
}

#[test]
fn updates_oldest_owner_when_message_history_is_long() {
    // Mix of messages; owner is buried in the middle.
    let mut messages = vec![
        Message::assistant("earlier"),
        assistant_with_tool_calls("owner", vec![tool_call("tc-X", "read", ToolStatus::Running)]),
        Message::assistant("middle unrelated"),
        assistant_with_tool_calls(
            "other agent",
            vec![tool_call("tc-Y", "grep", ToolStatus::Running)],
        ),
        Message::assistant("latest"),
    ];

    let update = tool_call("tc-X", "read", ToolStatus::Completed);
    route_tool_call_update(&mut messages, update);

    assert_eq!(messages[1].tool_calls[0].status, ToolStatus::Completed);
    // The sibling tool_call on message[3] must not be affected.
    assert_eq!(messages[3].tool_calls[0].status, ToolStatus::Running);
}

#[test]
fn unknown_tool_call_id_attaches_to_newest_message() {
    // Fallback path: IpcEvent::ToolCall arrived before its owning Message
    // (protocol anomaly). The update should still be preserved, not lost.
    let mut messages = vec![
        Message::assistant("first"),
        Message::assistant("latest"),
    ];

    let orphan = tool_call("tc-ORPHAN", "bash", ToolStatus::Running);
    route_tool_call_update(&mut messages, orphan);

    assert!(messages[0].tool_calls.is_empty());
    assert_eq!(messages[1].tool_calls.len(), 1);
    assert_eq!(messages[1].tool_calls[0].id, "tc-ORPHAN");
}

#[test]
fn unknown_tool_call_on_empty_history_is_dropped_silently() {
    // Defensive edge case: no messages at all. Handler should not panic.
    let mut messages: Vec<Message> = Vec::new();
    let orphan = tool_call("tc-ORPHAN", "bash", ToolStatus::Running);

    // No assertion on contents — just asserting this call does not panic.
    route_tool_call_update(&mut messages, orphan);
    assert!(messages.is_empty());
}

#[test]
fn update_replaces_existing_tool_call_fields_entirely() {
    let initial = ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({ "cmd": "ls" }),
        status: ToolStatus::Running,
        output: None,
        elapsed: None,
    };
    let mut messages = vec![assistant_with_tool_calls("w", vec![initial])];

    let completed = ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({ "cmd": "ls" }),
        status: ToolStatus::Completed,
        output: Some("file-a\nfile-b".to_string()),
        elapsed: Some(42),
    };
    route_tool_call_update(&mut messages, completed);

    let stored = &messages[0].tool_calls[0];
    assert_eq!(stored.status, ToolStatus::Completed);
    assert_eq!(stored.output.as_deref(), Some("file-a\nfile-b"));
    assert_eq!(stored.elapsed, Some(42));
}
