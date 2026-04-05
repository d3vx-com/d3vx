//! Layout State
//!
//! Encapsulates all layout tracking state for the application.
//! This struct was extracted from the main App struct to reduce the God Object pattern.

use ratatui::layout::Rect;

/// Layout tracking state extracted from App
///
/// Contains all fields related to tracking rendered layout positions
/// for mouse interaction and click detection. This allows layout state
/// to be tested in isolation and reduces coupling in the main App struct.
pub struct LayoutState {
    // ════════════════════════════════════════════════════════════════════════════
    // Sidebar Layout Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Row-to-workspace mapping for sidebar mouse interaction
    pub left_sidebar_workspace_rows: Vec<Option<usize>>,
    /// Row-to-agent mapping for sidebar mouse interaction
    pub sidebar_agent_rows: Vec<usize>,

    // ════════════════════════════════════════════════════════════════════════════
    // Chat Layout Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Line indices of agent rows in chat area for mouse interaction
    pub chat_agent_y_positions: Vec<(usize, usize)>, // (line_index, agent_index)
    /// Total lines in chat for click detection
    pub chat_total_lines: usize,

    // ════════════════════════════════════════════════════════════════════════════
    // Activity Panel Layout Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Y positions of agent rows in activity panel (relative to content area top)
    pub activity_agent_y_positions: Vec<usize>,
    /// Y positions of changed-file rows in activity panel (relative to content area top)
    pub activity_diff_y_positions: Vec<usize>,

    // ════════════════════════════════════════════════════════════════════════════
    // UI Component Layout Tracking (Rect regions for click detection)
    // ════════════════════════════════════════════════════════════════════════════
    /// Last rendered left sidebar rect for click detection
    pub last_left_sidebar_rect: Rect,
    /// Last rendered right sidebar rect for click detection
    pub last_right_sidebar_rect: Rect,
    /// Last rendered input rect for click detection
    pub last_input_rect: Rect,
    /// Last rendered chat rect for click detection
    pub last_chat_rect: Rect,
    /// Last rendered activity panel rect for click detection
    pub last_activity_rect: Option<Rect>,
    /// Last rendered selected-agent detail rect for click detection and scrolling
    pub last_agent_detail_rect: Option<Rect>,
    /// Last rendered tab bar rect for click detection
    pub last_tab_bar_rect: Option<Rect>,
    /// Last rendered mode bar rect for click detection
    pub last_mode_bar_rect: Option<Rect>,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            left_sidebar_workspace_rows: Vec::new(),
            sidebar_agent_rows: Vec::new(),
            chat_agent_y_positions: Vec::new(),
            chat_total_lines: 0,
            activity_agent_y_positions: Vec::new(),
            activity_diff_y_positions: Vec::new(),
            last_left_sidebar_rect: Rect::default(),
            last_right_sidebar_rect: Rect::default(),
            last_input_rect: Rect::default(),
            last_chat_rect: Rect::default(),
            last_activity_rect: None,
            last_agent_detail_rect: None,
            last_tab_bar_rect: None,
            last_mode_bar_rect: None,
        }
    }
}

impl LayoutState {
    /// Reset all layout tracking
    pub fn clear(&mut self) {
        self.left_sidebar_workspace_rows.clear();
        self.sidebar_agent_rows.clear();
        self.chat_agent_y_positions.clear();
        self.chat_total_lines = 0;
        self.activity_agent_y_positions.clear();
        self.activity_diff_y_positions.clear();
        self.last_left_sidebar_rect = Rect::default();
        self.last_right_sidebar_rect = Rect::default();
        self.last_input_rect = Rect::default();
        self.last_chat_rect = Rect::default();
        self.last_activity_rect = None;
        self.last_agent_detail_rect = None;
        self.last_tab_bar_rect = None;
        self.last_mode_bar_rect = None;
    }

    /// Check if any layout is being tracked
    pub fn has_layout_tracking(&self) -> bool {
        self.last_activity_rect.is_some()
            || self.last_agent_detail_rect.is_some()
            || self.last_tab_bar_rect.is_some()
            || self.last_mode_bar_rect.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_state_default() {
        let state = LayoutState::default();
        assert!(state.left_sidebar_workspace_rows.is_empty());
        assert!(state.sidebar_agent_rows.is_empty());
        assert!(state.chat_agent_y_positions.is_empty());
        assert_eq!(state.chat_total_lines, 0);
        assert!(state.activity_agent_y_positions.is_empty());
        assert!(state.activity_diff_y_positions.is_empty());
        assert!(state.last_activity_rect.is_none());
        assert!(state.last_agent_detail_rect.is_none());
        assert!(state.last_tab_bar_rect.is_none());
        assert!(state.last_mode_bar_rect.is_none());
    }

    #[test]
    fn test_has_layout_tracking() {
        let state = LayoutState::default();
        assert!(!state.has_layout_tracking());

        let mut state = LayoutState::default();
        state.last_activity_rect = Some(Rect::default());
        assert!(state.has_layout_tracking());
    }

    #[test]
    fn test_clear() {
        let mut state = LayoutState::default();
        state.left_sidebar_workspace_rows.push(Some(1));
        state.sidebar_agent_rows.push(1);
        state.chat_agent_y_positions.push((1, 1));
        state.chat_total_lines = 10;
        state.activity_agent_y_positions.push(1);
        state.activity_diff_y_positions.push(1);
        state.last_activity_rect = Some(Rect::default());
        state.last_agent_detail_rect = Some(Rect::default());
        state.last_tab_bar_rect = Some(Rect::default());
        state.last_mode_bar_rect = Some(Rect::default());

        state.clear();

        assert!(state.left_sidebar_workspace_rows.is_empty());
        assert!(state.sidebar_agent_rows.is_empty());
        assert!(state.chat_agent_y_positions.is_empty());
        assert_eq!(state.chat_total_lines, 0);
        assert!(state.activity_agent_y_positions.is_empty());
        assert!(state.activity_diff_y_positions.is_empty());
        assert!(state.last_activity_rect.is_none());
        assert!(state.last_agent_detail_rect.is_none());
        assert!(state.last_tab_bar_rect.is_none());
        assert!(state.last_mode_bar_rect.is_none());
    }
}
