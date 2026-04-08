//! App Module - Main Application State Machine
//!
//! Handles the main application loop, state management,
//! and coordination between UI, IPC, and Agent.
//!
//! Logic is split across submodules:
//! - `init`              : App constructor and startup wiring
//! - `event_forwarder`   : Agent event relay into main event loop
//! - `inline_agents`     : Inline agent UI state management
//! - `parallel_batches`  : Parallel batch restoration and graph summaries
//! - `runner`            : Main event loop, rendering, Drop cleanup
//! - `task_views`        : Task view refresh and selection
//! - `update`            : Periodic state refresh, notifications, polling

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;

use crate::ipc::{IpcClient, IpcHandle, ThinkingState};
use crate::mcp::McpManager;
use crate::ui::widgets::board::KanbanTask;

// ── Submodules ──────────────────────────────────────────────
pub mod actions;
pub mod agent;
pub mod commands;
pub mod dashboard_bridge;
pub mod event_forwarder;
pub mod handlers;
pub mod init;
pub mod inline_agents;
pub mod parallel_batches;
pub mod runner;
pub mod session_state_persister;
pub mod sessions;
pub mod slash_commands;
pub mod state;
pub mod task_views;
pub mod ui;
pub mod update;
pub mod vex_agent_poller;

#[cfg(test)]
mod tests;

// ── Re-exports ──────────────────────────────────────────────
pub use state::{
    AgentState, AppMode, CandidateEvaluation, FocusMode, InlineAgentInfo, InlineAgentStatus,
    InlineAgentUpdate, LayoutState, ParallelBatchState, ParallelChildStatus, ParallelChildTask,
    RightPaneTab, SessionState, ToolExecutionState, ToolState, UIState, WorkspaceStatus,
    WorkspaceTask, WorkspaceType,
};

// ── Types ───────────────────────────────────────────────────

/// In-memory state for a specific workspace
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub messages: Vec<crate::ipc::Message>,
    pub session_id: Option<String>,
    pub streaming_message: String,
    pub thinking: ThinkingState,
}

/// Application state
pub struct App {
    /// Working directory (active)
    pub cwd: Option<String>,
    /// Base working directory (original startup)
    pub base_cwd: Option<String>,
    /// Model to use
    pub model: Option<String>,

    /// Global configuration
    pub config: crate::config::D3vxConfig,

    /// UI state (extracted for modularity)
    pub ui: UIState,
    /// Session state (extracted for modularity - Phase D)
    pub session: SessionState,
    /// Agent state (extracted for modularity - Phase E)
    pub agents: AgentState,
    /// Tool state (extracted for modularity - Phase F)
    pub tools: ToolState,

    // ── Workspace/Git Data ──────────────────────────────────
    /// List of worktree tasks for left sidebar
    pub workspaces: Vec<WorkspaceTask>,
    pub workspace_selected_index: usize,
    /// Git changes for right sidebar
    pub git_changes: Vec<crate::app::state::FileChange>,
    /// Active branch name
    pub active_branch: String,
    /// PR number
    pub pr_number: Option<String>,

    // ── Animation State ─────────────────────────────────────
    /// Current animation frame (for glimmer effect)
    pub animation_frame: u64,
    /// Last update time
    pub last_update: Instant,
    /// Model Registry
    pub registry: Arc<tokio::sync::RwLock<crate::providers::ModelRegistry>>,
    /// Last git status refresh
    pub last_git_refresh: Instant,
    /// Last workspace refresh
    pub last_workspace_refresh: Instant,
    /// Last orchestrator status refresh
    pub last_orchestrator_refresh: Instant,
    /// MCP manager
    pub mcp_manager: Arc<McpManager>,

    // ── IPC ─────────────────────────────────────────────────
    /// IPC client
    pub ipc_client: Option<IpcClient>,
    /// IPC handle
    pub ipc_handle: Option<IpcHandle>,
    /// Primary LLM provider (for spawning sub-agents)
    pub provider: Option<Arc<dyn crate::providers::Provider>>,

    /// Event sender for self-loops
    pub event_tx: Option<mpsc::Sender<crate::event::Event>>,

    /// Database handle for persistence
    pub db: Option<crate::store::database::DatabaseHandle>,

    /// Sub-agent manager
    pub subagents: Arc<crate::agent::SubAgentManager>,

    // ── Right Pane Tab ──────────────────────────────────────
    /// Focused tab in the right-side operator console
    pub selected_right_pane_tab: RightPaneTab,

    // ── Flags ───────────────────────────────────────────────
    /// Should quit
    pub should_quit: bool,
    /// Counter for consecutive Ctrl+C presses (for force quit)
    pub ctrl_c_count: u8,
    /// Timestamp of last Ctrl+C press (for debounce)
    pub last_ctrl_c_time: Option<Instant>,

    // ── Command Palette ─────────────────────────────────────
    /// Command palette search filter
    pub command_palette_filter: String,
    /// Command palette selected index
    pub command_palette_selected: usize,

    // ── Diff View ───────────────────────────────────────────
    /// Current diff view content
    pub diff_view: Option<crate::ui::widgets::DiffView>,
    /// Live diff preview for the active workspace
    pub diff_preview: Option<crate::ui::widgets::DiffView>,
    /// Selected changed-file index for the live diff preview
    pub selected_diff_index: usize,

    // ── Undo Picker ─────────────────────────────────────────
    /// Undo picker state
    pub undo_picker: Option<crate::ui::widgets::UndoPicker>,
    /// Session picker state
    pub session_picker: Option<crate::ui::widgets::SessionPicker>,

    // ── Autonomous Mode ─────────────────────────────────────
    /// Whether autonomous mode is enabled
    pub autonomous_mode: bool,
    /// Number of autonomous iterations remaining
    pub autonomous_iterations: u32,

    // ── Services ────────────────────────────────────────────
    /// Symbol extractor for code analysis
    pub symbols: crate::services::SymbolExtractor,
    /// Memory search for FTS5 indexing
    pub memory_search: Option<crate::services::MemorySearch>,
    /// Permission manager for formal permission lifecycle
    pub permission_manager: Option<Arc<crate::pipeline::permission::PermissionManager>>,

    /// Kanban board state
    pub board: crate::ui::widgets::board::Board,

    /// Pipeline orchestrator for autonomous tasks
    pub orchestrator: Arc<crate::pipeline::PipelineOrchestrator>,

    // Layout state (extracted - Phase G migration)
    pub layout: LayoutState,

    /// Cache of states for all visited workspaces
    pub workspace_states: HashMap<String, WorkspaceState>,
    /// Cached list of active background tasks/workspaces from orchestrator
    pub background_active_tasks: Vec<(String, String)>,
    /// Cached queue statistics for the agent monitor
    pub background_queue_stats: crate::pipeline::QueueStats,
    /// Cached worker-pool statistics for the agent monitor
    pub background_worker_stats: crate::pipeline::WorkerPoolStats,
    /// Cached task rows for board/list views
    pub task_view_tasks: Vec<KanbanTask>,
    /// Cached full task records for inspector/detail views
    pub task_view_records: Vec<crate::store::task::Task>,
    /// Selected task index in list view
    pub list_selected_task: usize,
    /// Transient notifications (toasts)
    pub notifications: Vec<crate::app::state::Notification>,

    // ── Dashboard ───────────────────────────────────────────
    /// Optional dashboard server for SSE streaming
    pub dashboard: Option<crate::pipeline::dashboard::Dashboard>,
}
