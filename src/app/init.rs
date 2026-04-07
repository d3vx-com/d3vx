//! Application Initialization
//!
//! Handles creating the App instance, setting up IPC, tool coordinator,
//! agent loops, MCP servers, and other startup wiring.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::agent::ToolCoordinator;
use crate::app::state::{
    AgentState, AppMode, LayoutState, RightPaneTab, SessionState, ToolState, UIState,
};
use crate::app::App;
use crate::ipc::IpcClient;
use crate::mcp::McpManager;
use tracing::{debug, info, warn};

impl App {
    /// Create a new app
    pub async fn new(
        cwd: Option<String>,
        model: Option<String>,
        session_id: Option<String>,
        stream_out: Option<std::path::PathBuf>,
        resume: bool,
        dashboard: Option<crate::pipeline::dashboard::Dashboard>,
    ) -> Result<Self> {
        let is_standalone = std::env::var("D3VX_TUI_MODE").ok().as_deref() == Some("standalone");
        let config = crate::config::load_config(crate::config::LoadConfigOptions::default())?;

        // Only create IPC client if not in standalone mode
        let (ipc_client, ipc_handle) = if is_standalone {
            (None, None)
        } else {
            let (client, handle) = IpcClient::new();
            (Some(client), Some(handle))
        };

        // Initialize model registry
        let registry = Arc::new(tokio::sync::RwLock::new(
            crate::providers::ModelRegistry::new(),
        ));
        let registry_clone = registry.clone();
        tokio::spawn(async move {
            let mut reg = registry_clone.write().await;
            debug!("Starting background model discovery...");
            if let Err(e) = reg.discover_all().await {
                warn!("Initial model discovery failed: {}", e);
            } else {
                info!("Initial model discovery completed.");
            }
        });

        // Create tool coordinator and register tools
        let tool_coordinator = Arc::new(ToolCoordinator::new());
        let mcp_manager = Arc::new(McpManager::new());

        // Initialize database BEFORE orchestrator
        let db = crate::store::database::Database::open_default()
            .ok()
            .map(|d| Arc::new(parking_lot::Mutex::new(d)));
        let mut orch_config = crate::pipeline::OrchestratorConfig::default();
        // Use a persistent directory for checkpoints
        orch_config.checkpoint_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".d3vx/checkpoints");

        orch_config.github = config.integrations.as_ref().and_then(|i| i.github.clone());
        let orchestrator =
            Arc::new(crate::pipeline::PipelineOrchestrator::new(orch_config, db.clone()).await?);

        // Register core tools
        tool_coordinator
            .register_tool(crate::tools::BashTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::ReadTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::WriteTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::EditTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::GlobTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::GrepTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::ThinkTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::QuestionTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::TodoWriteTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::WebFetchTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::MultiEditTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::DelegateReviewTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::CompleteTaskTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::WebSearchTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::NotebookEditTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::ListMcpResourcesTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::ReadMcpResourceTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::CronCreateTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::CronDeleteTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::CronListTool::new())
            .await;
        // Background tasks
        tool_coordinator
            .register_tool(crate::tools::TaskOutputTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::TaskStopTool::new())
            .await;
        // Worktree
        tool_coordinator
            .register_tool(crate::tools::WorktreeCreateTool::new())
            .await;
        tool_coordinator
            .register_tool(crate::tools::WorktreeRemoveTool::new())
            .await;

        // Set up SpawnParallelTool with a channel for events
        let (spawn_tx, spawn_rx) = tokio::sync::mpsc::channel(32);
        let spawn_parallel_tool = crate::tools::SpawnParallelTool::with_sender(spawn_tx);
        tool_coordinator.register_tool(spawn_parallel_tool).await;
        tool_coordinator
            .register_tool(crate::tools::MultiStrategyTool::new())
            .await;

        // Initialize agent loop for standalone mode
        let (agent_loop, agent_events, init_hint) = if is_standalone {
            let permission_manager = Some(Arc::new(
                crate::pipeline::permission::PermissionManager::new(),
            ));
            let (agent, events, hint) = Self::create_agent(
                &cwd,
                &model,
                &session_id,
                tool_coordinator.clone(),
                false,
                true,
                crate::app::state::FocusMode::Chat,
                permission_manager,
            )?;
            if let Some(agent) = &agent {
                let orch = orchestrator.clone();
                let agent = agent.clone();
                tokio::spawn(async move {
                    orch.set_agent(agent).await;
                });
            }
            (agent, events, hint)
        } else {
            (None, None, None)
        };

        let mut workspace_agents = HashMap::new();
        let mut pending_agent_receivers = HashMap::new();
        if let Some(agent) = agent_loop.clone() {
            workspace_agents.insert("home".to_string(), agent);
        }
        if let Some(events) = agent_events {
            // Wire dashboard bridge if dashboard is available
            if let Some(ref dash) = dashboard {
                let session = session_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                let bridge_rx = events.resubscribe();
                crate::app::dashboard_bridge::DashboardBridge::spawn(
                    dash.clone(),
                    bridge_rx,
                    session,
                );
            }
            pending_agent_receivers.insert("home".to_string(), events);
        }

        // Check if agent was created successfully
        let is_connected = agent_loop.is_some();

        // Extract provider if available from standalone loop initialization
        let provider = if is_standalone {
            Self::create_provider(&config).ok()
        } else {
            None
        };

        let mut subagents = crate::agent::SubAgentManager::new();
        if let Some(db_handle) = db.clone() {
            subagents = subagents.with_db(db_handle);
        }
        let subagents = Arc::new(subagents);

        // Extract UI config values before `config` is moved into the struct
        let ui_mode = match config.ui.mode {
            crate::config::types::UiMode::Chat => AppMode::Chat,
            crate::config::types::UiMode::Kanban => AppMode::Board,
            crate::config::types::UiMode::List => AppMode::List,
            crate::config::types::UiMode::Suggestion => AppMode::Chat,
        };
        let ui_power_mode = config.ui.power_mode;
        let ui_show_welcome = config.ui.show_welcome;
        let ui_sidebar_width = config.ui.sidebar_width;

        let model = model.or_else(|| Some(config.model.clone()));

        let mut app = Self {
            cwd: cwd.clone(),
            base_cwd: cwd,
            model,
            config,
            subagents,
            provider,
            db,

            // UI state (extracted - Phase B migration complete)
            ui: UIState {
                mode: ui_mode,
                show_welcome: ui_show_welcome,
                power_mode: ui_power_mode,
                sidebar_width: ui_sidebar_width,
                ..UIState::default()
            },

            // Session state (extracted - Phase D migration)
            session: SessionState {
                session_id: session_id.clone(),
                home_session_id: session_id,
                stream_out,
                init_hint,
                ..SessionState::default()
            },

            // Agent state (extracted - Phase E migration)
            agents: AgentState {
                is_connected,
                workspace_agents,
                pending_agent_receivers,
                agent_loop,
                streaming_message: String::new(),
                inline_agents: Vec::new(),
                selected_inline_agent: None,
                parallel_agents_enabled: true,
                parallel_batches: HashMap::new(),
                spawn_parallel_receiver: Some(spawn_rx),
                pending_agent_queue: Vec::new(),
                running_parallel_agents: 0,
                active_parallel_batches: 0,
            },

            // Tool state (extracted - Phase F migration)
            tools: ToolState::new(tool_coordinator),

            autonomous_mode: false,
            autonomous_iterations: 0,

            symbols: crate::services::SymbolExtractor::new(),
            memory_search: {
                let db_path = crate::store::database::Database::default_db_path();
                crate::services::MemorySearch::new(db_path.to_string_lossy().as_ref()).ok()
            },
            permission_manager: Some(Arc::new(
                crate::pipeline::permission::PermissionManager::new(),
            )),

            board: crate::ui::widgets::board::Board::new(),

            workspaces: Vec::new(),
            workspace_selected_index: 0,
            git_changes: Vec::new(),
            active_branch: String::from("main"), // default
            pr_number: None,
            notifications: Vec::new(),

            // Layout state (extracted - Phase G migration)
            layout: LayoutState::default(),

            animation_frame: 0,
            last_update: Instant::now(),
            registry,
            last_git_refresh: Instant::now(),
            last_workspace_refresh: Instant::now(),
            last_orchestrator_refresh: Instant::now(),

            ipc_client,
            ipc_handle,
            event_tx: None,

            selected_right_pane_tab: RightPaneTab::Agent,

            should_quit: false,
            ctrl_c_count: 0,
            last_ctrl_c_time: None,

            command_palette_filter: String::new(),
            command_palette_selected: 0,

            diff_view: None,
            diff_preview: None,
            selected_diff_index: 0,
            undo_picker: None,
            session_picker: None,
            orchestrator,
            workspace_states: HashMap::new(),
            background_active_tasks: Vec::new(),
            background_queue_stats: crate::pipeline::QueueStats::default(),
            background_worker_stats: crate::pipeline::WorkerPoolStats::default(),
            task_view_tasks: Vec::new(),
            task_view_records: Vec::new(),
            list_selected_task: 0,
            mcp_manager,
            dashboard,
        };

        // Initial data fetch
        let _ = app.refresh_git_status();
        let _ = app.refresh_workspaces();
        let _ = app.refresh_task_views();

        // If --resume flag was passed, open session picker immediately
        if resume && is_standalone {
            if let Some(db_handle) = app.db.clone() {
                let db = db_handle.lock();
                let store = crate::store::session::SessionStore::from_connection(db.connection());
                let options = crate::store::session::SessionListOptions {
                    project_path: app.cwd.clone(),
                    limit: Some(20),
                    ..Default::default()
                };
                if let Ok(sessions) = store.list(options) {
                    if sessions.is_empty() {
                        info!("No previous sessions found for --resume");
                    } else {
                        info!("Opening session picker for --resume ({} sessions)", sessions.len());
                        app.session_picker = Some(crate::ui::widgets::SessionPicker::new(sessions));
                        app.ui.mode = AppMode::SessionPicker;
                    }
                }
            }
        }

        Ok(app)
    }

    pub fn apply_initial_ui_mode(&mut self, ui_mode: Option<&str>) {
        match ui_mode {
            Some("kanban") => self.ui.mode = AppMode::Board,
            Some("list") => self.ui.mode = AppMode::List,
            _ => {}
        }
    }
}
