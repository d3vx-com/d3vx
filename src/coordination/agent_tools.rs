//! Tool bindings that expose the coordination layer to an agent.
//!
//! Five tools, each a thin wrapper over a board or inbox operation:
//!
//! | Tool                    | Backed by                               |
//! |-------------------------|-----------------------------------------|
//! | `coord_list_ready_tasks`| `CoordinationBoard::list_ready_tasks`   |
//! | `coord_claim_task`      | `CoordinationBoard::claim_task`         |
//! | `coord_complete_task`   | `CoordinationBoard::complete_task`      |
//! | `coord_send_message`    | `Inbox::send` (recipient's inbox)       |
//! | `coord_drain_inbox`     | `Inbox::drain` (this agent's inbox)     |
//!
//! The toolset is built from a coordination root + agent id; the root
//! layout is `{root}/tasks/` for the board and `{root}/inboxes/` for
//! per-agent inboxes. Callers that want richer coordination (broadcast
//! announcements, task creation) call the underlying
//! [`CoordinationBoard`](super::CoordinationBoard) and
//! [`BroadcastLog`](super::BroadcastLog) directly — those aren't
//! exposed as tools yet because the common worker flow doesn't need
//! them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::board::CoordinationBoard;
use super::errors::CoordinationError;
use super::inbox::{Inbox, Message};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Bundle of coordination tools for one agent, pointed at one root.
///
/// Cheap to clone: internal state is `Arc`-shared, so the returned
/// tools can be distributed across async tasks freely.
#[derive(Clone)]
pub struct CoordinationToolset {
    board: Arc<CoordinationBoard>,
    inbox: Arc<Inbox>,
    inboxes_dir: PathBuf,
    agent_id: String,
}

impl CoordinationToolset {
    /// Build a toolset rooted at `coord_root`. Creates the board
    /// (`tasks/`) and inbox directory (`inboxes/`) if they don't exist
    /// already.
    pub fn new(
        coord_root: impl AsRef<Path>,
        agent_id: impl Into<String>,
    ) -> Result<Self, CoordinationError> {
        let root = coord_root.as_ref();
        let board = Arc::new(CoordinationBoard::open(root.join("tasks"))?);
        let inboxes_dir = root.join("inboxes");
        let agent_id = agent_id.into();
        let inbox = Arc::new(Inbox::open(&inboxes_dir, &agent_id)?);
        Ok(Self {
            board,
            inbox,
            inboxes_dir,
            agent_id,
        })
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Produce the tool instances. Registers each on the tool
    /// coordinator is the caller's responsibility.
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(ListReadyTasksTool {
                board: self.board.clone(),
            }),
            Arc::new(ClaimTaskTool {
                board: self.board.clone(),
                agent_id: self.agent_id.clone(),
            }),
            Arc::new(CompleteTaskTool {
                board: self.board.clone(),
            }),
            Arc::new(SendMessageTool {
                inboxes_dir: self.inboxes_dir.clone(),
                from: self.agent_id.clone(),
            }),
            Arc::new(DrainInboxTool {
                inbox: self.inbox.clone(),
            }),
        ]
    }
}

// ── tool impls ────────────────────────────────────────────────────

struct ListReadyTasksTool {
    board: Arc<CoordinationBoard>,
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

struct ClaimTaskTool {
    board: Arc<CoordinationBoard>,
    agent_id: String,
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

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) else {
            return ToolResult::error("missing `task_id`");
        };
        match self.board.claim_task(task_id, &self.agent_id) {
            Ok(t) => ToolResult::success(
                serde_json::to_string_pretty(&t).unwrap_or_default(),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

struct CompleteTaskTool {
    board: Arc<CoordinationBoard>,
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

struct SendMessageTool {
    inboxes_dir: PathBuf,
    from: String,
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

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
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
        let msg = Message::new(&self.from, to, body);
        match inbox.send(&msg) {
            Ok(()) => ToolResult::success(format!("sent to {to}")),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

struct DrainInboxTool {
    inbox: Arc<Inbox>,
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

    async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        match self.inbox.drain() {
            Ok(messages) => ToolResult::success(
                serde_json::to_string_pretty(&messages).unwrap_or_else(|_| "[]".into()),
            ),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}
