//! State Field Application
//!
//! Internal helper to apply an agent event to specific message/thinking/
//! streaming state fields without requiring an App reference. Used by both
//! the main and workspace event handlers.

use anyhow::Result;

use crate::agent::AgentEvent;
use crate::ipc::{Message, MessageRole, ThinkingState, ToolStatus};

/// Apply an agent event to state fields (messages, thinking, streaming_message).
pub fn apply_event_to_state_fields(
    event: AgentEvent,
    messages: &mut Vec<Message>,
    thinking: &mut ThinkingState,
    streaming_message: &mut String,
) -> Result<()> {
    match event {
        AgentEvent::Text { text } => {
            streaming_message.push_str(&text);
            let should_create = match messages.last() {
                Some(last) if last.role == MessageRole::Assistant && last.is_streaming => false,
                _ => true,
            };
            if should_create {
                messages.push(Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: MessageRole::Assistant,
                    content: streaming_message.clone(),
                    timestamp: chrono::Utc::now(),
                    is_error: false,
                    tool_calls: Vec::new(),
                    is_streaming: true,
                    shell_cmd: None,
                    exit_code: None,
                });
            } else if let Some(last) = messages.last_mut() {
                last.content = streaming_message.clone();
            }
        }
        AgentEvent::Thinking { text } => {
            *thinking = ThinkingState {
                is_thinking: true,
                text,
                phase: crate::ipc::types::ThinkingPhase::Thinking,
            };
        }
        AgentEvent::ToolStart { id, name } => {
            let should_create = match messages.last() {
                Some(last) if last.role == MessageRole::Assistant => false,
                _ => true,
            };
            if should_create {
                messages.push(Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: MessageRole::Assistant,
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                    is_error: false,
                    tool_calls: Vec::new(),
                    is_streaming: true,
                    shell_cmd: None,
                    exit_code: None,
                });
            }
            if let Some(last) = messages.last_mut() {
                last.tool_calls.push(crate::ipc::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: serde_json::Value::Null,
                    output: None,
                    status: ToolStatus::Running,
                    elapsed: None,
                });
            }
            let tool_display_name = match name.as_str() {
                "BashTool" | "Bash" => "Running shell command...".to_string(),
                _ => format!("Running {}...", name),
            };
            *thinking = ThinkingState {
                is_thinking: true,
                text: tool_display_name,
                phase: crate::ipc::types::ThinkingPhase::Thinking,
            };
        }
        AgentEvent::ToolInput { json } => {
            if let Some(last) = messages.last_mut() {
                if let Some(tc) = last.tool_calls.last_mut() {
                    tc.input = serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                }
            }
        }
        AgentEvent::ToolEnd {
            id,
            result,
            is_error,
            elapsed_ms,
            ..
        } => {
            if let Some(msg) = messages
                .iter_mut()
                .rev()
                .find(|m| m.role == MessageRole::Assistant)
            {
                if let Some(tc) = msg.tool_calls.iter_mut().find(|tc| tc.id == id) {
                    tc.output = Some(result.clone());
                    tc.status = if is_error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Completed
                    };
                    tc.elapsed = Some(elapsed_ms);
                }
            }
        }
        AgentEvent::MessageEnd { .. } => {
            *thinking = ThinkingState {
                is_thinking: true,
                text: "Thinking...".to_string(),
                phase: crate::ipc::types::ThinkingPhase::Thinking,
            };
        }
        AgentEvent::Error { error } => {
            messages.push(Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: MessageRole::Assistant,
                content: format!("Error: {}", error),
                timestamp: chrono::Utc::now(),
                is_error: true,
                tool_calls: Vec::new(),
                is_streaming: false,
                shell_cmd: None,
                exit_code: None,
            });
            *thinking = ThinkingState::default();
        }
        AgentEvent::Done { .. } => {
            *thinking = ThinkingState::default();
            streaming_message.clear();
            if let Some(last) = messages.last_mut() {
                if last.role == MessageRole::Assistant {
                    last.is_streaming = false;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
