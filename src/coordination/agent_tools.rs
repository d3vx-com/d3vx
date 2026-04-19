//! Tool bindings that expose the coordination layer to an agent.
//!
//! This file owns the **factory** surface — `CoordinationToolset` and
//! its `register_on` convenience. The actual `Tool` implementations
//! live in [`tool_impls`](super::tool_impls) so this file stays under
//! the 300-line guideline.
//!
//! # Tools
//!
//! | Tool                     | Backed by                              |
//! |--------------------------|----------------------------------------|
//! | `coord_list_ready_tasks` | `CoordinationBoard::list_ready_tasks`  |
//! | `coord_claim_task`       | `CoordinationBoard::claim_task`        |
//! | `coord_complete_task`    | `CoordinationBoard::complete_task`     |
//! | `coord_send_message`     | `Inbox::send` (recipient's inbox)      |
//! | `coord_drain_inbox`      | `Inbox::drain` (this agent's inbox)    |
//!
//! # Per-agent context via `ToolContext`
//!
//! Tools are registered **once** on a shared
//! [`ToolCoordinator`](crate::agent::ToolCoordinator). Every agent that
//! executes a tool provides its own
//! [`ToolContext`](crate::tools::ToolContext); the calling agent's id
//! is pulled from `ctx.session_id` at call time, so one registered set
//! serves every agent — no name collisions, no per-agent tool
//! lifetimes.
//!
//! # Root layout
//!
//! ```text
//! {coord_root}/
//!   ├── tasks/                 ← CoordinationBoard data
//!   └── inboxes/
//!       ├── {agent_id}.jsonl   ← Inbox::open resolves this
//!       └── ...
//! ```
//!
//! Callers that want richer coordination (broadcast announcements, task
//! creation) reach into [`CoordinationBoard`](super::CoordinationBoard)
//! and [`BroadcastLog`](super::BroadcastLog) directly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::board::CoordinationBoard;
use super::errors::CoordinationError;
use super::prompt::coordination_preamble;
use super::tool_impls::{
    ClaimTaskTool, CompleteTaskTool, DrainInboxTool, ListReadyTasksTool, SendMessageTool,
};
use crate::agent::{AgentConfig, ToolCoordinator};
use crate::tools::{Tool, ToolContext, ToolResult};

/// Bundle of coordination tools rooted at one coordination directory.
///
/// Cheap to clone — internal state is `Arc`-shared. Agent id is
/// resolved from [`ToolContext::session_id`] at tool-call time, so one
/// toolset serves every agent using the same coordination root.
#[derive(Clone)]
pub struct CoordinationToolset {
    board: Arc<CoordinationBoard>,
    inboxes_dir: PathBuf,
}

impl CoordinationToolset {
    /// Build a toolset rooted at `coord_root`. Creates `tasks/` and
    /// `inboxes/` under the root if they don't already exist.
    pub fn new(coord_root: impl AsRef<Path>) -> Result<Self, CoordinationError> {
        let root = coord_root.as_ref();
        let board = Arc::new(CoordinationBoard::open(root.join("tasks"))?);
        let inboxes_dir = root.join("inboxes");
        std::fs::create_dir_all(&inboxes_dir).map_err(|source| {
            CoordinationError::Io {
                path: inboxes_dir.clone(),
                source,
            }
        })?;
        Ok(Self { board, inboxes_dir })
    }

    /// Produce the tool instances as `Arc<dyn Tool>`. Useful for
    /// enumeration or custom registration paths; ordinary callers
    /// should prefer [`register_on`](Self::register_on).
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(ListReadyTasksTool {
                board: self.board.clone(),
            }),
            Arc::new(ClaimTaskTool {
                board: self.board.clone(),
            }),
            Arc::new(CompleteTaskTool {
                board: self.board.clone(),
            }),
            Arc::new(SendMessageTool {
                inboxes_dir: self.inboxes_dir.clone(),
            }),
            Arc::new(DrainInboxTool {
                inboxes_dir: self.inboxes_dir.clone(),
            }),
        ]
    }

    /// One-liner spawn-time integration: register every coordination
    /// tool on `coordinator` **and** prepend the coordination preamble
    /// to `config.system_prompt` so the agent knows the tools exist.
    ///
    /// An empty existing system prompt is replaced outright; a
    /// non-empty one has the preamble inserted ahead of it with a
    /// blank-line separator. This keeps any task-specific persona the
    /// caller has already configured.
    pub async fn attach_to_agent(
        &self,
        coordinator: &ToolCoordinator,
        config: &mut AgentConfig,
    ) {
        self.register_on(coordinator).await;
        let preamble = coordination_preamble();
        if config.system_prompt.is_empty() {
            config.system_prompt = preamble;
        } else {
            config.system_prompt = format!("{preamble}\n{}", config.system_prompt);
        }
    }

    /// Register every coordination tool on `coordinator`. Call once at
    /// startup (or at spawn time) after building the toolset — tools
    /// are agent-agnostic, so one registration covers every agent that
    /// will use the same coordinator.
    pub async fn register_on(&self, coordinator: &ToolCoordinator) {
        coordinator
            .register_tool(ListReadyTasksTool {
                board: self.board.clone(),
            })
            .await;
        coordinator
            .register_tool(ClaimTaskTool {
                board: self.board.clone(),
            })
            .await;
        coordinator
            .register_tool(CompleteTaskTool {
                board: self.board.clone(),
            })
            .await;
        coordinator
            .register_tool(SendMessageTool {
                inboxes_dir: self.inboxes_dir.clone(),
            })
            .await;
        coordinator
            .register_tool(DrainInboxTool {
                inboxes_dir: self.inboxes_dir.clone(),
            })
            .await;
    }
}

/// Pull the calling agent's id from the tool context, or produce a
/// clear error result if it's absent.
///
/// `pub(super)` so sibling `tool_impls` can call it but nothing outside
/// the coordination module sees it.
pub(super) fn require_agent_id(ctx: &ToolContext) -> Result<&str, ToolResult> {
    ctx.session_id.as_deref().ok_or_else(|| {
        ToolResult::error(
            "ToolContext has no session_id; coordination tools require one",
        )
    })
}
