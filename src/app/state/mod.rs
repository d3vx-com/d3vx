//! App State Types Module
//!
//! Organized into modular components:
//! - `types` - Core type definitions (AppMode, FocusMode, InlineAgentInfo, etc.)
//! - `ui_state` - UI-related state (extracted from App to reduce God Object pattern)
//! - `session_state` - Session-related state (messages, tokens, costs)
//! - `agent_state` - Agent-related state (agent loops, inline agents, parallel execution)
//! - `tool_state` - Tool execution state (coordinator, executing tools, recent tools)
//! - `layout_state` - Layout tracking state (rectangles, row mappings, y_positions)
//! - `inline_agent` - Inline agent types and parallel batch state

mod agent_state;
mod inline_agent;
mod layout_state;
mod session_state;
mod tool_state;
mod types;
mod ui_state;

// Re-export all public types for backward compatibility
pub use agent_state::AgentState;
pub use inline_agent::{
    CandidateEvaluation, InlineAgentInfo, InlineAgentStatus, InlineAgentUpdate, ParallelBatchState,
    ParallelChildStatus, ParallelChildTask,
};
pub use layout_state::LayoutState;
pub use session_state::SessionState;
pub use tool_state::ToolState;
pub use types::*;
pub use ui_state::UIState;
