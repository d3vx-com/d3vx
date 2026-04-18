//! Internal `Tool` implementations backing the public
//! [`CoordinationToolset`](super::agent_tools::CoordinationToolset).
//!
//! Kept in a sibling file so the factory can stay under the 300-line
//! guideline. Every struct here is `pub(super)` — only the factory may
//! construct them. Agents never see these types directly; they see the
//! [`Tool`] trait via the registered coordinator.
//!
//! Each struct owns only what it needs: a board handle, or the inboxes
//! directory. None carry an `agent_id` — that's resolved from
//! [`ToolContext::session_id`] on every call via
//! [`super::agent_tools::require_agent_id`].

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::agent_tools::require_agent_id;
use super::board::CoordinationBoard;
use super::inbox::{Inbox, Message};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

pub(super) struct ListReadyTasksTool {
    pub(super) board: Arc<CoordinationBoard>,
}

#[async_trait]
impl Tool for ListReadyTasksTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "coord_list_ready_tasks".to_string(),
            description: "List tasks on the coordination board whose \
                dependencies are satisfied and which have no current \
                owner. Use before starting new work to avoid \
                duplicating another agent's effort.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        match self.board.list_ready_tasks() {
            Ok(tasks) => ToolResult::success(
                serde_json::to_string_pretty(&tasks).unwrap_or_else(|_| "[]".into()),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub(super) struct ClaimTaskTool {
    pub(super) board: Arc<CoordinationBoard>,
}

#[async_trait]
impl Tool for ClaimTaskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "coord_claim_task".to_string(),
            description: "Atomically claim a ready task for this agent. \
                Fails if the task is already claimed, not in \
                Pending state, or has unresolved dependencies."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string", "description": "Task id to claim." }
                },
                "required": ["task_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let agent_id = match require_agent_id(ctx) {
            Ok(id) => id,
            Err(r) => return r,
        };
        let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `task_id`");
        };
        match self.board.claim_task(task_id, agent_id) {
            Ok(t) => ToolResult::success(
                serde_json::to_string_pretty(&t).unwrap_or_default(),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub(super) struct CompleteTaskTool {
    pub(super) board: Arc<CoordinationBoard>,
}

#[async_trait]
impl Tool for CompleteTaskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "coord_complete_task".to_string(),
            description: "Mark a task as completed with a short result \
                summary. Only the current owner should call this."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" },
                    "result":  { "type": "string", "description": "Short summary of what was done." }
                },
                "required": ["task_id", "result"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `task_id`");
        };
        let Some(result) = input.get("result").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `result`");
        };
        match self.board.complete_task(task_id, result) {
            Ok(t) => ToolResult::success(
                serde_json::to_string_pretty(&t).unwrap_or_default(),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub(super) struct SendMessageTool {
    pub(super) inboxes_dir: PathBuf,
}

#[async_trait]
impl Tool for SendMessageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "coord_send_message".to_string(),
            description: "Send a point-to-point message to another \
                agent's inbox. The recipient reads via \
                coord_drain_inbox.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to":   { "type": "string", "description": "Recipient agent id." },
                    "body": { "type": "string", "description": "Message body." }
                },
                "required": ["to", "body"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let from = match require_agent_id(ctx) {
            Ok(id) => id,
            Err(r) => return r,
        };
        let Some(to) = input.get("to").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `to`");
        };
        let Some(body) = input.get("body").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `body`");
        };
        let inbox = match Inbox::open(&self.inboxes_dir, to) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let msg = Message::new(from, to, body);
        match inbox.send(&msg) {
            Ok(()) => ToolResult::success(format!("sent to {to}")),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub(super) struct DrainInboxTool {
    pub(super) inboxes_dir: PathBuf,
}

#[async_trait]
impl Tool for DrainInboxTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "coord_drain_inbox".to_string(),
            description: "Read and clear every message addressed to this \
                agent. Returns a JSON array of messages (from, body, \
                sent_at). Call at the start of each iteration."
                .to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn execute(&self, _input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let agent_id = match require_agent_id(ctx) {
            Ok(id) => id,
            Err(r) => return r,
        };
        let inbox = match Inbox::open(&self.inboxes_dir, agent_id) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        match inbox.drain() {
            Ok(messages) => ToolResult::success(
                serde_json::to_string_pretty(&messages).unwrap_or_else(|_| "[]".into()),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}
