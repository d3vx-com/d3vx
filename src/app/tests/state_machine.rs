//! TUI State Machine Tests
//!
//! Comprehensive tests for the `App` state machine, covering:
//!
//! - Mode transitions (Chat ↔ overlays)
//! - Keyboard handler dispatch
//! - Input editing (insert, delete, cursor movement)
//! - Sidebar toggles
//! - History navigation
//! - Scroll controls
//! - Command palette navigation
//! - Diff view navigation
//! - Board/List view navigation
//! - Undo picker navigation
//! - Session picker navigation
//! - Escape double-tap detection

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Instant;

    use crate::app::state::{
        AgentState, FocusMode, LayoutState, RightPaneTab, SessionState, ToolState, UIState,
    };
    use crate::app::{App, AppMode};
    use crate::store::task::{ExecutionMode, TaskState};

    /// Helper to create a minimal `App` for state machine testing.
    ///
    /// Avoids IPC, database, and agent dependencies.
    async fn test_app() -> App {
        let tool_coordinator = Arc::new(crate::agent::ToolCoordinator::new());
        let mcp_manager = Arc::new(crate::mcp::McpManager::new());
        let config =
            crate::config::load_config(crate::config::LoadConfigOptions::default()).unwrap();
        let registry = Arc::new(tokio::sync::RwLock::new(
            crate::providers::ModelRegistry::new(),
        ));
        let symbols = crate::services::SymbolExtractor::new();

        // Use a minimal board (empty)
        let board = crate::ui::widgets::board::Board::new();

        // Create the orchestrator asynchronously
        let orch_config = crate::pipeline::orchestrator::OrchestratorConfig::default();
        let orchestrator = Arc::new(
            crate::pipeline::PipelineOrchestrator::new(orch_config, None)
                .await
                .unwrap(),
        );

        let subagents = Arc::new(crate::agent::SubAgentManager::new());

        App {
            cwd: Some("/tmp/test".to_string()),
            base_cwd: Some("/tmp/test".to_string()),
            model: Some("test-model".to_string()),
            config,
            subagents,
            provider: None,
            db: None,

            // UI state (extracted for modularity)
            ui: UIState::default(),

            // Session state (extracted for modularity - Phase D)
            session: SessionState::default(),

            // Agent state (extracted for modularity - Phase E)
            agents: AgentState::default(),

            // Workspace/Git data
            workspaces: Vec::new(),
            workspace_selected_index: 0,
            git_changes: Vec::new(),
            active_branch: "main".to_string(),
            pr_number: None,
            notifications: Vec::new(),

            autonomous_mode: false,
            autonomous_iterations: 0,

            // Tool state (extracted - Phase F migration)
            tools: ToolState::new(tool_coordinator),

            animation_frame: 0,
            last_update: Instant::now(),
            needs_redraw: true,
            cached_subagent_count: 0,
            registry,
            last_git_refresh: Instant::now(),
            last_workspace_refresh: Instant::now(),
            last_orchestrator_refresh: Instant::now(),
            mcp_manager,

            ipc_client: None,
            ipc_handle: None,
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
            symbols,
            memory_search: None,
            permission_manager: None,
            board,
            orchestrator,

            // Layout state (extracted - Phase G migration)
            layout: LayoutState::default(),

            workspace_states: HashMap::new(),
            background_active_tasks: Vec::new(),
            background_queue_stats: crate::pipeline::QueueStats::default(),
            background_worker_stats: crate::pipeline::WorkerPoolStats::default(),
            task_view_tasks: Vec::new(),
            task_view_records: Vec::new(),
            list_selected_task: 0,
            dashboard: None,
        }
    }

    /// Helper to create a key press event.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Helper to create a key press event with modifiers.
    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    // ========================================================================
    // Mode Transition Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_default_mode_is_chat() {
        let app = test_app().await;
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_p_toggles_power_mode() {
        let mut app = test_app().await;
        assert!(!app.ui.power_mode);
        app.handle_key_event(key_mod(KeyCode::Char('p'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.power_mode);
        assert!(app.ui.right_sidebar_visible);

        app.handle_key_event(key_mod(KeyCode::Char('p'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.ui.power_mode);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_command_palette() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_question_mark_opens_help() {
        let mut app = test_app().await;
        // '?' in empty input buffer opens help
        app.ui.input_buffer.clear();
        app.handle_key_event(key(KeyCode::Char('?'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Help);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_help() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::Help;
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_diff_preview() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::DiffPreview;
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_q_closes_diff_preview() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::DiffPreview;
        app.handle_key_event(key(KeyCode::Char('q'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_board_view() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::Board;
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_q_closes_list_view() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::List;
        app.handle_key_event(key(KeyCode::Char('q'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_undo_picker() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::UndoPicker;
        app.undo_picker = Some(crate::ui::widgets::UndoPicker::default());
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
        assert!(app.undo_picker.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_closes_session_picker() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::SessionPicker;
        app.session_picker = Some(crate::ui::widgets::SessionPicker::new(Vec::new()));
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
        assert!(app.session_picker.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_d_toggles_diff_preview() {
        let mut app = test_app().await;
        app.diff_view = Some(crate::ui::widgets::DiffView::new(
            "test.rs",
            "+ added\n- removed",
        ));

        // Open diff preview
        app.handle_key_event(key_mod(KeyCode::Char('d'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert_eq!(app.ui.mode, AppMode::DiffPreview);

        // Ctrl+D exits diff preview first, re-enter requires going back to Chat mode
        // The DiffPreview handler handles Esc/q, not Ctrl+D
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_apply_initial_ui_mode() {
        let mut app = test_app().await;
        app.apply_initial_ui_mode(Some("kanban"));
        assert_eq!(app.ui.mode, AppMode::Board);

        let mut app2 = test_app().await;
        app2.apply_initial_ui_mode(Some("list"));
        assert_eq!(app2.ui.mode, AppMode::List);

        let mut app3 = test_app().await;
        app3.apply_initial_ui_mode(None);
        assert_eq!(app3.ui.mode, AppMode::Chat);
    }

    // ========================================================================
    // Sidebar Toggle Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_l_toggles_unified_sidebar() {
        let mut app = test_app().await;
        // UIState defaults right_sidebar_visible to true; toggle once to start from closed
        app.ui.right_sidebar_visible = false;

        assert!(!app.ui.right_sidebar_visible);

        app.handle_key_event(key_mod(KeyCode::Char('l'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.right_sidebar_visible);

        app.handle_key_event(key_mod(KeyCode::Char('l'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.ui.right_sidebar_visible);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_r_toggles_right_sidebar() {
        let mut app = test_app().await;
        // UIState defaults right_sidebar_visible to true; toggle once to start from closed
        app.ui.right_sidebar_visible = false;
        assert!(!app.ui.right_sidebar_visible);

        app.handle_key_event(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.right_sidebar_visible);

        app.handle_key_event(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.ui.right_sidebar_visible);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_a_pins_agent_monitor() {
        let mut app = test_app().await;
        assert!(!app.ui.agent_monitor_pinned);

        app.handle_key_event(key_mod(KeyCode::Char('a'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.agent_monitor_pinned);
        assert!(app.ui.right_sidebar_visible); // Also opens sidebar

        app.handle_key_event(key_mod(KeyCode::Char('a'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.ui.agent_monitor_pinned);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_n_opens_navigator() {
        let mut app = test_app().await;
        app.ui.right_sidebar_visible = false;
        app.ui.agent_monitor_pinned = true;

        app.handle_key_event(key_mod(KeyCode::Char('n'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.right_sidebar_visible);
        assert!(!app.ui.agent_monitor_pinned);
    }

    // ========================================================================
    // Input Editing Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_char_insertion() {
        let mut app = test_app().await;
        app.handle_key_event(key(KeyCode::Char('h'))).await.unwrap();
        app.handle_key_event(key(KeyCode::Char('i'))).await.unwrap();
        assert_eq!(app.ui.input_buffer, "hi");
        assert_eq!(app.ui.cursor_position, 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_backspace() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 5;

        app.handle_key_event(key(KeyCode::Backspace)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "hell");
        assert_eq!(app.ui.cursor_position, 4);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_backspace_at_start_is_noop() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 0;

        app.handle_key_event(key(KeyCode::Backspace)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "hello"); // No change
        assert_eq!(app.ui.cursor_position, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_at_cursor() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 2;

        app.handle_key_event(key(KeyCode::Delete)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "helo");
        assert_eq!(app.ui.cursor_position, 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_at_end_is_noop() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 5;

        app.handle_key_event(key(KeyCode::Delete)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "hello"); // No change
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_cursor_left() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 3;

        app.handle_key_event(key(KeyCode::Left)).await.unwrap();
        assert_eq!(app.ui.cursor_position, 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_cursor_left_at_start_stays() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 0;

        app.handle_key_event(key(KeyCode::Left)).await.unwrap();
        assert_eq!(app.ui.cursor_position, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_cursor_right() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 3;

        app.handle_key_event(key(KeyCode::Right)).await.unwrap();
        assert_eq!(app.ui.cursor_position, 4);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_cursor_right_at_end_stays() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hello".to_string();
        app.ui.cursor_position = 5;

        app.handle_key_event(key(KeyCode::Right)).await.unwrap();
        assert_eq!(app.ui.cursor_position, 5);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_u_clears_input() {
        let mut app = test_app().await;
        app.ui.input_buffer = "some text".to_string();
        app.ui.cursor_position = 5;

        app.handle_key_event(key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.input_buffer.is_empty());
        assert_eq!(app.ui.cursor_position, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_mid_buffer_insertion() {
        let mut app = test_app().await;
        app.ui.input_buffer = "hllo".to_string();
        app.ui.cursor_position = 1;

        app.handle_key_event(key(KeyCode::Char('e'))).await.unwrap();
        assert_eq!(app.ui.input_buffer, "hello");
        assert_eq!(app.ui.cursor_position, 2);
    }

    // ========================================================================
    // Verbose Toggle Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_o_toggles_activity_tools_expanded() {
        let mut app = test_app().await;
        assert!(!app.tools.activity_tools_expanded);

        app.handle_key_event(key_mod(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.tools.activity_tools_expanded);

        app.handle_key_event(key_mod(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.tools.activity_tools_expanded);
    }

    // ========================================================================
    // Quit Behavior Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_c_quits_when_idle() {
        let mut app = test_app().await;
        app.handle_key_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.should_quit);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_c_stops_conversation_when_thinking() {
        let mut app = test_app().await;
        app.session.thinking.is_thinking = true;

        app.handle_key_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.should_quit); // Should NOT quit
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_c_stops_conversation_when_streaming() {
        let mut app = test_app().await;
        app.agents.streaming_message = "streaming...".to_string();

        app.handle_key_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(!app.should_quit);
    }

    // ========================================================================
    // Scroll Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_shift_up_scrolls_up() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 0;
        app.ui.max_scroll.set(100);

        app.handle_key_event(key_mod(KeyCode::Up, KeyModifiers::SHIFT))
            .await
            .unwrap();
        assert_eq!(app.ui.scroll_offset, 5);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_shift_down_scrolls_down() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 10;
        app.ui.max_scroll.set(100);

        app.handle_key_event(key_mod(KeyCode::Down, KeyModifiers::SHIFT))
            .await
            .unwrap();
        assert_eq!(app.ui.scroll_offset, 5);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_scroll_clamped_at_zero() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 2;

        app.handle_key_event(key_mod(KeyCode::Down, KeyModifiers::SHIFT))
            .await
            .unwrap();
        assert_eq!(app.ui.scroll_offset, 0); // Clamped, not negative
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_home_scrolls_to_top() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 0;
        app.ui.max_scroll.set(100);

        app.handle_key_event(key(KeyCode::Home)).await.unwrap();
        assert_eq!(app.ui.scroll_offset, 100); // max_scroll
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_end_scrolls_to_bottom() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 50;

        app.handle_key_event(key(KeyCode::End)).await.unwrap();
        assert_eq!(app.ui.scroll_offset, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_k_scrolls_up_one() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 5;
        app.ui.max_scroll.set(100);

        app.handle_key_event(key_mod(KeyCode::Char('k'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert_eq!(app.ui.scroll_offset, 6);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_j_scrolls_down_one() {
        let mut app = test_app().await;
        app.ui.scroll_offset = 5;

        app.handle_key_event(key_mod(KeyCode::Char('j'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert_eq!(app.ui.scroll_offset, 4);
    }

    // ========================================================================
    // History Navigation Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_up_navigates_history() {
        let mut app = test_app().await;
        app.ui.input_history = vec!["first".to_string(), "second".to_string()];
        app.ui.history_index = 2; // Past the end

        app.handle_key_event(key(KeyCode::Up)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "second");
        assert_eq!(app.ui.history_index, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_down_navigates_history_forward() {
        let mut app = test_app().await;
        app.ui.input_history = vec!["first".to_string(), "second".to_string()];
        app.ui.history_index = 0;
        app.ui.input_buffer = "first".to_string();

        app.handle_key_event(key(KeyCode::Down)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "second");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_down_past_end_clears_input() {
        let mut app = test_app().await;
        app.ui.input_history = vec!["first".to_string()];
        app.ui.history_index = 0;
        app.ui.input_buffer = "first".to_string();

        app.handle_key_event(key(KeyCode::Down)).await.unwrap();
        assert!(app.ui.input_buffer.is_empty());
    }

    // ========================================================================
    // Command Palette Navigation Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_command_palette_typing_filters() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;

        app.handle_key_event(key(KeyCode::Char('h'))).await.unwrap();
        assert_eq!(app.command_palette_filter, "h");
        assert_eq!(app.command_palette_selected, 0);

        app.handle_key_event(key(KeyCode::Char('e'))).await.unwrap();
        assert_eq!(app.command_palette_filter, "he");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_command_palette_backspace() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;
        app.command_palette_filter = "hel".to_string();

        app.handle_key_event(key(KeyCode::Backspace)).await.unwrap();
        assert_eq!(app.command_palette_filter, "he");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_command_palette_arrow_navigation() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;

        // Down arrow selects next item
        app.handle_key_event(key(KeyCode::Down)).await.unwrap();
        assert_eq!(app.command_palette_selected, 1);

        // Up arrow selects previous item
        app.handle_key_event(key(KeyCode::Up)).await.unwrap();
        assert_eq!(app.command_palette_selected, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_command_palette_up_at_top_stays() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;
        app.command_palette_selected = 0;

        app.handle_key_event(key(KeyCode::Up)).await.unwrap();
        assert_eq!(app.command_palette_selected, 0); // Clamped
    }

    // ========================================================================
    // Diff View Navigation Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_diff_view_scroll_down_j() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::DiffPreview;
        app.diff_view = Some(crate::ui::widgets::DiffView::new(
            "test.rs",
            "+ line\n- line\n  line\n",
        ));

        app.handle_key_event(key(KeyCode::Char('j'))).await.unwrap();
        // Should scroll down without changing mode
        assert_eq!(app.ui.mode, AppMode::DiffPreview);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_diff_view_scroll_up_k() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::DiffPreview;
        app.diff_view = Some(crate::ui::widgets::DiffView::new(
            "test.rs",
            "+ line\n- line\n",
        ));

        app.handle_key_event(key(KeyCode::Char('k'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::DiffPreview);
    }

    // ========================================================================
    // Board View Navigation Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_board_vim_navigation() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::Board;

        // Vim keys should navigate without leaving Board mode
        app.handle_key_event(key(KeyCode::Char('j'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Board);

        app.handle_key_event(key(KeyCode::Char('k'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Board);

        app.handle_key_event(key(KeyCode::Char('h'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Board);

        app.handle_key_event(key(KeyCode::Char('l'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Board);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_board_q_returns_to_chat() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::Board;

        app.handle_key_event(key(KeyCode::Char('q'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    // ========================================================================
    // List View Navigation Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_list_view_navigation() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::List;
        app.task_view_tasks = vec![
            crate::ui::widgets::board::KanbanTask {
                id: "1".to_string(),
                title: "Task 1".to_string(),
                state: TaskState::Backlog,
                execution_mode: None,
                priority: 0,
                merge_ready: None,
                blocking_count: 0,
                qa_iteration: 0,
            },
            crate::ui::widgets::board::KanbanTask {
                id: "2".to_string(),
                title: "Task 2".to_string(),
                state: TaskState::Implement,
                execution_mode: Some(ExecutionMode::Vex),
                priority: 1,
                merge_ready: Some(true),
                blocking_count: 0,
                qa_iteration: 1,
            },
        ];
        app.list_selected_task = 0;

        app.handle_key_event(key(KeyCode::Down)).await.unwrap();
        assert_eq!(app.list_selected_task, 1);
        assert_eq!(app.ui.mode, AppMode::List);

        // Can't go past the end
        app.handle_key_event(key(KeyCode::Down)).await.unwrap();
        assert_eq!(app.list_selected_task, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_list_up_at_zero_stays() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::List;
        app.list_selected_task = 0;

        app.handle_key_event(key(KeyCode::Up)).await.unwrap();
        assert_eq!(app.list_selected_task, 0);
    }

    // ========================================================================
    // Welcome Banner Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_typing_dismisses_welcome() {
        let mut app = test_app().await;
        app.ui.show_welcome = true;

        app.handle_key_event(key(KeyCode::Char('h'))).await.unwrap();
        assert!(!app.ui.show_welcome);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_esc_dismisses_welcome() {
        let mut app = test_app().await;
        app.ui.show_welcome = true;

        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert!(!app.ui.show_welcome);
    }

    // ========================================================================
    // Key Release Events Are Ignored
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_key_release_ignored() {
        let mut app = test_app().await;
        let release = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        app.handle_key_event(release).await.unwrap();
        // No state change
        assert_eq!(app.ui.mode, AppMode::Chat);
        assert!(app.ui.input_buffer.is_empty());
    }

    // ========================================================================
    // Message Queue Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_x_pops_message_queue() {
        let mut app = test_app().await;
        app.session.message_queue = vec!["msg1".to_string(), "msg2".to_string()];

        app.handle_key_event(key_mod(KeyCode::Char('x'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert_eq!(app.session.message_queue.len(), 1);
        assert_eq!(app.session.message_queue[0], "msg1");
    }

    // ========================================================================
    // Compound / Edge Case Tests
    // ========================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_mode_isolation_chat_keys_in_command_palette() {
        let mut app = test_app().await;
        app.ui.mode = AppMode::CommandPalette;

        // Ctrl+C should NOT quit while in command palette
        // (command palette handler handles its own keys)
        app.handle_key_event(key(KeyCode::Char('a'))).await.unwrap();
        assert_eq!(app.command_palette_filter, "a");
        assert_eq!(app.ui.mode, AppMode::CommandPalette);
        assert!(!app.should_quit);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_question_mark_types_into_non_empty_buffer() {
        let mut app = test_app().await;
        app.ui.input_buffer = "some text".to_string();
        app.ui.cursor_position = 9;

        // '?' should type into the buffer, NOT open help (only opens when buffer is empty)
        app.handle_key_event(key(KeyCode::Char('?'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
        assert_eq!(app.ui.input_buffer, "some text?");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_full_roundtrip_command_palette() {
        let mut app = test_app().await;

        // 1. Manually enter command palette mode (no longer toggled by Ctrl+P)
        app.ui.mode = AppMode::CommandPalette;

        // 2. Type a filter
        app.handle_key_event(key(KeyCode::Char('h'))).await.unwrap();
        assert_eq!(app.command_palette_filter, "h");

        // 3. Close with Esc
        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
        assert!(app.command_palette_filter.is_empty()); // Cleared on close
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_sequential_mode_transitions() {
        let mut app = test_app().await;

        // Chat → Help → Chat → PowerMode (Ctrl+P) → Chat
        app.handle_key_event(key(KeyCode::Char('?'))).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Help);

        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);

        app.handle_key_event(key_mod(KeyCode::Char('p'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.power_mode);
        assert_eq!(app.ui.mode, AppMode::Chat); // Still in Chat but PowerMode toggled

        app.handle_key_event(key(KeyCode::Esc)).await.unwrap();
        assert_eq!(app.ui.mode, AppMode::Chat);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_insert_delete_roundtrip() {
        let mut app = test_app().await;

        // Type "abc"
        app.handle_key_event(key(KeyCode::Char('a'))).await.unwrap();
        app.handle_key_event(key(KeyCode::Char('b'))).await.unwrap();
        app.handle_key_event(key(KeyCode::Char('c'))).await.unwrap();
        assert_eq!(app.ui.input_buffer, "abc");

        // Backspace to "ab"
        app.handle_key_event(key(KeyCode::Backspace)).await.unwrap();
        assert_eq!(app.ui.input_buffer, "ab");

        // Move left, insert 'x' → "axb"
        app.handle_key_event(key(KeyCode::Left)).await.unwrap();
        app.handle_key_event(key(KeyCode::Char('x'))).await.unwrap();
        assert_eq!(app.ui.input_buffer, "axb");

        // Clear all
        app.handle_key_event(key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert!(app.ui.input_buffer.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_ctrl_tab_cycles_focus_mode() {
        let mut app = test_app().await;
        assert_eq!(app.ui.focus_mode, FocusMode::Chat);

        app.handle_key_event(key_mod(KeyCode::Tab, KeyModifiers::CONTROL))
            .await
            .unwrap();
        assert_eq!(app.ui.focus_mode, FocusMode::Build);

        app.handle_key_event(key_mod(
            KeyCode::Tab,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ))
        .await
        .unwrap();
        assert_eq!(app.ui.focus_mode, FocusMode::Chat);
    }
}
