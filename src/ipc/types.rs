//! IPC Types - Shared between TUI and Agent
//!
//! These types must match the TypeScript definitions in src/tui/types.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ────────────────────────────────────────────────────────────
// Message Types
// ────────────────────────────────────────────────────────────

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Shell,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
            Self::Shell => write!(f, "shell"),
        }
    }
}

/// A conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Whether the message is currently being streamed (enables glimmer effect)
    #[serde(default)]
    pub is_streaming: bool,
    /// Shell-specific fields (role === 'shell')
    pub shell_cmd: Option<String>,
    pub exit_code: Option<i32>,
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            is_error: false,
            tool_calls: Vec::new(),
            is_streaming: false,
            shell_cmd: None,
            exit_code: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            is_error: false,
            tool_calls: Vec::new(),
            is_streaming: false,
            shell_cmd: None,
            exit_code: None,
        }
    }

    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            is_error: false,
            tool_calls: Vec::new(),
            is_streaming: false,
            shell_cmd: None,
            exit_code: None,
        }
    }

    /// Create a new error message (system role with is_error flag)
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            is_error: true,
            tool_calls: Vec::new(),
            is_streaming: false,
            shell_cmd: None,
            exit_code: None,
        }
    }

    /// Create a new shell message
    pub fn shell(cmd: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Shell,
            content: content.into(),
            timestamp: Utc::now(),
            is_error: false,
            tool_calls: Vec::new(),
            is_streaming: false,
            shell_cmd: Some(cmd.into()),
            exit_code: None,
        }
    }
}

// ────────────────────────────────────────────────────────────
// Tool Call Types
// ────────────────────────────────────────────────────────────

/// Tool call status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Pending,
    Running,
    Completed,
    Error,
    WaitingApproval,
}

/// A tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub status: ToolStatus,
    pub output: Option<String>,
    pub elapsed: Option<u64>,
}

// ────────────────────────────────────────────────────────────
// Thinking State
// ────────────────────────────────────────────────────────────

/// Thinking phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingPhase {
    #[default]
    Tokenizing,
    Dispatching,
    Parsing,
    Thinking,
    // Vex Mode Phases
    Research,
    Plan,
    Draft,
    Implement,
    Review,
    Docs,
}

/// Current thinking state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThinkingState {
    #[serde(default)]
    pub is_thinking: bool,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub phase: ThinkingPhase,
}

// ────────────────────────────────────────────────────────────
// Token Usage
// ────────────────────────────────────────────────────────────

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub total_cost: Option<f64>,
}

// ────────────────────────────────────────────────────────────
// Permission Request
// ────────────────────────────────────────────────────────────

/// Permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub workspace_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub action: String,
    pub resource: Option<String>,
    pub message: String,
    pub diff: Option<String>,
    pub options: Vec<PermissionOption>,
}

/// Permission option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOption {
    pub label: String,
    pub value: String,
    pub is_default: bool,
}

/// Decision for a command approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Allow the tool to execute
    Approve,
    /// Deny the tool execution
    Deny,
    /// Allow all future tools in this session (Trust Mode)
    ApproveAll,
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Hello, world!");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello, world!"));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{
            "id": "test-id",
            "role": "assistant",
            "content": "Hello!",
            "timestamp": "2024-01-01T00:00:00Z",
            "is_streaming": true
        }"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, MessageRole::Assistant);
        assert!(msg.is_streaming);
    }
}
