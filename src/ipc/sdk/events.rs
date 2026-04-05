//! SDK Event Types
//!
//! Event and response types for SDK mode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SdkEvent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    Error {
        error: String,
    },
    Thinking {
        thinking: String,
    },
    Done {
        summary: Option<String>,
    },
    PermissionRequest {
        request: PermissionRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool: String,
    pub command: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SdkResponse {
    Input { message: String },
    PermissionResponse { request_id: String, approved: bool },
    Interrupt,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    Initialize { model: Option<String> },
    SetModel { model: String },
    SetPermissionMode { mode: String },
    Interrupt,
    Resume,
    SetMaxThinkingTokens { tokens: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Initialized { session_id: String },
    ModelChanged { model: String },
    Interrupted,
    Resumed,
    Error { error: String },
}
