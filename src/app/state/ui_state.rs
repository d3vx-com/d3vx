//! UI State
//!
//! Encapsulates all UI-related state for the application.
//! This struct was extracted from the main App struct to reduce the God Object pattern.

use std::cell::Cell;
use std::time::Instant;

use crate::app::state::FocusMode;
use crate::app::state::RightPaneTab;
use crate::app::AppMode;
use crate::ui::theme::Theme;

/// Double-tap escape threshold in milliseconds
const ESCAPE_DOUBLE_TAP_MS: u64 = 500;

/// UI-related state extracted from App
///
/// Contains all fields related to display, input handling, and UI state management.
/// This allows UI state to be tested in isolation and reduces coupling in the main App struct.
#[derive(Debug)]
pub struct UIState {
    // ════════════════════════════════════════════════════════════════════════════
    // Mode and Display Settings
    // ════════════════════════════════════════════════════════════════════════════
    /// Current application mode
    pub mode: AppMode,
    /// Read-only plan mode (blocks write tools)
    pub plan_mode: bool,
    /// Verbose mode (expanded tool calls)
    pub verbose: bool,
    /// Power Mode (advanced telemetry)
    pub power_mode: bool,
    /// Show welcome banner
    pub show_welcome: bool,
    /// Current theme
    pub theme: Theme,

    // ════════════════════════════════════════════════════════════════════════════
    // Scroll State
    // ════════════════════════════════════════════════════════════════════════════
    /// Scroll offset for message list
    pub scroll_offset: usize,
    /// Activity panel scroll offset
    pub activity_scroll_offset: usize,
    /// Total lines in activity panel content
    pub activity_content_lines: usize,
    /// Scroll offset for selected agent transcript/details in the right pane
    pub selected_agent_output_scroll: usize,
    /// Total rendered lines in the selected agent transcript/details
    pub selected_agent_output_lines: usize,
    /// Maximum scroll bound cached during render
    pub max_scroll: Cell<usize>,
    /// Scroll offset for help modal
    pub help_scroll: usize,

    // ════════════════════════════════════════════════════════════════════════════
    // Input State
    // ════════════════════════════════════════════════════════════════════════════
    /// Input buffer
    pub input_buffer: String,
    /// Input cursor position
    pub cursor_position: usize,
    /// Whether we are in multiline input mode (waiting for next line after \)
    pub multiline_pending: bool,
    /// Lightweight chat focus preset shown above the prompt
    pub focus_mode: FocusMode,

    // ════════════════════════════════════════════════════════════════════════════
    // Sidebar State
    // ════════════════════════════════════════════════════════════════════════════
    /// Right sidebar visibility
    pub right_sidebar_visible: bool,
    /// Whether the agent monitor is pinned open inside the navigator
    pub agent_monitor_pinned: bool,
    /// Sidebar width
    pub sidebar_width: u16,
    /// Focused tab in the right-side operator console
    pub selected_right_pane_tab: RightPaneTab,

    // ════════════════════════════════════════════════════════════════════════════
    // History State
    // ════════════════════════════════════════════════════════════════════════════
    /// Input history
    pub input_history: Vec<String>,
    /// History index
    pub history_index: usize,
    /// History search prefix (for typed prefix matching)
    pub history_prefix: Option<String>,

    // ════════════════════════════════════════════════════════════════════════════
    // Mention Completion
    // ════════════════════════════════════════════════════════════════════════════
    /// Active inline mention suggestions for @file completion
    pub mention_suggestions: Vec<String>,
    /// Selected mention suggestion index
    pub mention_selected: usize,

    // ════════════════════════════════════════════════════════════════════════════
    // Escape Key Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Escape key count for double-tap detection
    pub escape_count: u8,
    /// Last escape key press time
    pub last_escape_time: Instant,

    // ════════════════════════════════════════════════════════════════════════════
    // Command Palette
    // ════════════════════════════════════════════════════════════════════════════
    /// Command palette search filter
    pub command_palette_filter: String,
    /// Command palette selected index
    pub command_palette_selected: usize,

    // ════════════════════════════════════════════════════════════════════════════
    // Layout Tracking for Mouse
    // ════════════════════════════════════════════════════════════════════════════
    /// Last rendered left sidebar rect for click detection
    pub last_left_sidebar_rect: ratatui::layout::Rect,
    /// Last rendered right sidebar rect for click detection
    pub last_right_sidebar_rect: ratatui::layout::Rect,
    /// Last rendered input rect for click detection
    pub last_input_rect: ratatui::layout::Rect,
    /// Last rendered chat rect for click detection
    pub last_chat_rect: ratatui::layout::Rect,
    /// Last rendered activity panel rect for click detection
    pub last_activity_rect: Option<ratatui::layout::Rect>,
    /// Last rendered selected-agent detail rect for click detection and scrolling
    pub last_agent_detail_rect: Option<ratatui::layout::Rect>,
    /// Last rendered tab bar rect for click detection
    pub last_tab_bar_rect: Option<ratatui::layout::Rect>,
    /// Last rendered mode bar rect for click detection
    pub last_mode_bar_rect: Option<ratatui::layout::Rect>,

    // ════════════════════════════════════════════════════════════════════════════
    // Row Tracking for Mouse
    // ════════════════════════════════════════════════════════════════════════════
    /// Row-to-workspace mapping for sidebar mouse interaction
    pub left_sidebar_workspace_rows: Vec<Option<usize>>,
    /// Row-to-agent mapping for sidebar mouse interaction
    pub sidebar_agent_rows: Vec<usize>,
    /// Line indices of agent rows in chat area for mouse interaction (line_index, agent_index)
    pub chat_agent_y_positions: Vec<(usize, usize)>,
    /// Total lines in chat for click detection
    pub chat_total_lines: usize,
    /// Y positions of agent rows in activity panel (relative to content area top)
    pub activity_agent_y_positions: Vec<usize>,
    /// Y positions of changed-file rows in activity panel (relative to content area top)
    pub activity_diff_y_positions: Vec<usize>,

    // ════════════════════════════════════════════════════════════════════════════
    // Model Picker State
    // ════════════════════════════════════════════════════════════════════════════
    /// Whether the model picker modal is visible
    pub show_model_picker: bool,
    /// Currently focused tier in the picker (Simple, Standard, Complex)
    pub model_picker_selected_tier: crate::providers::ComplexityTier,
    /// Selected index within the current tier's model list
    pub model_picker_selected_index: usize,
    /// Filter string for model search
    pub model_picker_filter: String,
    /// Whether we are currently entering an API key
    pub model_picker_entering_api_key: bool,
    /// Current input for the API key prompt
    pub model_picker_api_key_input: String,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            mode: AppMode::Chat,
            plan_mode: false,
            verbose: false,
            power_mode: false,
            show_welcome: true,
            theme: Theme::default(),

            scroll_offset: 0,
            activity_scroll_offset: 0,
            activity_content_lines: 0,
            selected_agent_output_scroll: 0,
            selected_agent_output_lines: 0,
            max_scroll: Cell::new(0),
            help_scroll: 0,

            input_buffer: String::new(),
            cursor_position: 0,
            multiline_pending: false,
            focus_mode: FocusMode::default(),

            right_sidebar_visible: true,
            agent_monitor_pinned: false,
            sidebar_width: 35,
            selected_right_pane_tab: RightPaneTab::default(),

            input_history: Vec::new(),
            history_index: 0,
            history_prefix: None,

            mention_suggestions: Vec::new(),
            mention_selected: 0,

            escape_count: 0,
            last_escape_time: Instant::now(),

            command_palette_filter: String::new(),
            command_palette_selected: 0,

            last_left_sidebar_rect: ratatui::layout::Rect::default(),
            last_right_sidebar_rect: ratatui::layout::Rect::default(),
            last_input_rect: ratatui::layout::Rect::default(),
            last_chat_rect: ratatui::layout::Rect::default(),
            last_activity_rect: None,
            last_agent_detail_rect: None,
            last_tab_bar_rect: None,
            last_mode_bar_rect: None,

            left_sidebar_workspace_rows: Vec::new(),
            sidebar_agent_rows: Vec::new(),
            chat_agent_y_positions: Vec::new(),
            chat_total_lines: 0,
            activity_agent_y_positions: Vec::new(),
            activity_diff_y_positions: Vec::new(),

            show_model_picker: false,
            model_picker_selected_tier: crate::providers::ComplexityTier::Standard,
            model_picker_selected_index: 0,
            model_picker_filter: String::new(),
            model_picker_entering_api_key: false,
            model_picker_api_key_input: String::new(),
        }
    }
}

impl UIState {
    /// Create a new UIState with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an escape key press and return true if it's a double-tap
    pub fn record_escape(&mut self) -> bool {
        let now = Instant::now();
        let is_double = now.duration_since(self.last_escape_time)
            < std::time::Duration::from_millis(ESCAPE_DOUBLE_TAP_MS);
        self.escape_count += 1;
        self.last_escape_time = now;
        is_double
    }

    /// Reset escape tracking
    pub fn reset_escape(&mut self) {
        self.escape_count = 0;
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Remove character before cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            // Find the character boundary before cursor
            let prev_pos = self.cursor_position.saturating_sub(1);
            self.input_buffer.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    /// Navigate up through input history
    pub fn navigate_history_up(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(item) = self.input_history.get(self.history_index) {
                self.input_buffer = item.clone();
                self.cursor_position = self.input_buffer.len();
            }
        }
    }

    /// Navigate down through input history
    pub fn navigate_history_down(&mut self) {
        if self.history_index < self.input_history.len().saturating_sub(1) {
            self.history_index += 1;
            if let Some(item) = self.input_history.get(self.history_index) {
                self.input_buffer = item.clone();
                self.cursor_position = self.input_buffer.len();
            }
        } else {
            // At end of history, clear buffer
            self.history_index = self.input_history.len();
            self.input_buffer.clear();
            self.cursor_position = 0;
        }
    }

    /// Add current input to history and reset
    pub fn submit_input(&mut self) -> String {
        let input = std::mem::take(&mut self.input_buffer);
        if !input.is_empty() {
            self.input_history.push(input.clone());
        }
        self.cursor_position = 0;
        self.multiline_pending = false;
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_state_default() {
        let state = UIState::default();
        assert_eq!(state.mode, AppMode::Chat);
        assert!(!state.plan_mode);
        assert!(!state.verbose);
        assert!(state.input_buffer.is_empty());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_insert_char() {
        let mut state = UIState::default();
        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.input_buffer, "hi");
        assert_eq!(state.cursor_position, 2);
    }

    #[test]
    fn test_backspace() {
        let mut state = UIState::default();
        state.insert_char('h');
        state.insert_char('i');
        state.backspace();
        assert_eq!(state.input_buffer, "h");
        assert_eq!(state.cursor_position, 1);
    }

    #[test]
    fn test_history_navigation() {
        let mut state = UIState::default();
        state.input_history = vec!["cmd1".to_string(), "cmd2".to_string()];
        state.history_index = 2;

        state.navigate_history_up();
        assert_eq!(state.input_buffer, "cmd2");

        state.navigate_history_up();
        assert_eq!(state.input_buffer, "cmd1");
    }

    #[test]
    fn test_escape_tracking() {
        let mut state = UIState::default();
        // First escape - record it
        state.record_escape();
        assert_eq!(state.escape_count, 1);

        // Reset escape count
        state.reset_escape();
        assert_eq!(state.escape_count, 0);
    }
}
