//! Tool State
//!
//! Encapsulates all tool execution-related state for the application.
//! This struct was extracted from the main App struct to reduce the God Object pattern.

use std::collections::HashSet;
use std::sync::Arc;

use crate::agent::ToolCoordinator;
use crate::app::state::ToolExecutionState;

/// Tool execution-related state extracted from App
///
/// Contains all fields related to tool coordination, execution tracking,
/// and UI state for tool display. This allows tool state to be tested
/// in isolation and reduces coupling in the main App struct.
pub struct ToolState {
    // ════════════════════════════════════════════════════════════════════════════
    // Tool Coordination
    // ════════════════════════════════════════════════════════════════════════════
    /// Tool coordinator for executing tools
    pub tool_coordinator: Arc<ToolCoordinator>,

    // ════════════════════════════════════════════════════════════════════════════
    // Tool Execution Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Currently executing tools (by ID)
    pub executing_tools: Vec<ToolExecutionState>,
    /// Recent completed tools for activity panel
    pub recent_tools: Vec<ToolExecutionState>,
    /// Tool calls that are fully expanded in the UI
    pub expanded_tool_calls: HashSet<String>,

    // ════════════════════════════════════════════════════════════════════════════
    // Tool UI State
    // ════════════════════════════════════════════════════════════════════════════
    /// Whether tools section in activity panel is expanded
    pub activity_tools_expanded: bool,
    /// Whether standalone tool execution is enabled
    pub standalone_tools_enabled: bool,
}

impl ToolState {
    /// Create a new ToolState with the given tool coordinator
    pub fn new(tool_coordinator: Arc<ToolCoordinator>) -> Self {
        Self {
            tool_coordinator,
            executing_tools: Vec::new(),
            recent_tools: Vec::new(),
            expanded_tool_calls: HashSet::new(),
            activity_tools_expanded: false,
            standalone_tools_enabled: false,
        }
    }

    /// Check if any tools are currently executing
    pub fn has_executing_tools(&self) -> bool {
        !self.executing_tools.is_empty()
    }

    /// Get the count of executing tools
    pub fn executing_count(&self) -> usize {
        self.executing_tools.len()
    }

    /// Get the count of recent tools
    pub fn recent_count(&self) -> usize {
        self.recent_tools.len()
    }

    /// Check if a tool call is expanded in the UI
    pub fn is_expanded(&self, tool_id: &str) -> bool {
        self.expanded_tool_calls.contains(tool_id)
    }

    /// Toggle expansion state of a tool call
    pub fn toggle_expansion(&mut self, tool_id: &str) {
        if self.expanded_tool_calls.contains(tool_id) {
            self.expanded_tool_calls.remove(tool_id);
        } else {
            self.expanded_tool_calls.insert(tool_id.to_string());
        }
    }

    /// Clear all tool state (except coordinator)
    pub fn clear(&mut self) {
        self.executing_tools.clear();
        self.recent_tools.clear();
        self.expanded_tool_calls.clear();
        self.activity_tools_expanded = false;
        // Note: standalone_tools_enabled is NOT cleared as it's a config setting
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_state() -> ToolState {
        let coordinator = Arc::new(ToolCoordinator::new());
        ToolState::new(coordinator)
    }

    #[test]
    fn test_tool_state_new() {
        let state = create_test_state();
        assert!(state.executing_tools.is_empty());
        assert!(state.recent_tools.is_empty());
        assert!(state.expanded_tool_calls.is_empty());
        assert!(!state.activity_tools_expanded);
        assert!(!state.standalone_tools_enabled);
    }

    #[test]
    fn test_has_executing_tools() {
        let state = create_test_state();
        assert!(!state.has_executing_tools());
    }

    #[test]
    fn test_executing_count() {
        let state = create_test_state();
        assert_eq!(state.executing_count(), 0);
    }

    #[test]
    fn test_recent_count() {
        let state = create_test_state();
        assert_eq!(state.recent_count(), 0);
    }

    #[test]
    fn test_is_expanded() {
        let mut state = create_test_state();
        assert!(!state.is_expanded("tool-1"));

        state.expanded_tool_calls.insert("tool-1".to_string());
        assert!(state.is_expanded("tool-1"));
    }

    #[test]
    fn test_toggle_expansion() {
        let mut state = create_test_state();

        // First toggle - expand
        state.toggle_expansion("tool-1");
        assert!(state.is_expanded("tool-1"));

        // Second toggle - collapse
        state.toggle_expansion("tool-1");
        assert!(!state.is_expanded("tool-1"));
    }

    #[test]
    fn test_clear() {
        let mut state = create_test_state();
        // Note: ToolExecutionState doesn't implement Default due to Instant field
        // So we just test with empty vectors
        state.expanded_tool_calls.insert("tool-1".to_string());
        state.activity_tools_expanded = true;

        state.clear();

        assert!(state.executing_tools.is_empty());
        assert!(state.recent_tools.is_empty());
        assert!(state.expanded_tool_calls.is_empty());
        assert!(!state.activity_tools_expanded);
        // standalone_tools_enabled unchanged
        assert!(!state.standalone_tools_enabled);
    }
}
