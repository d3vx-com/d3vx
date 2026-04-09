//! Agent Core Module
//!
//! This module provides the core agent functionality for d3vx, including:
//!
//! - **Conversation management**: Track message history with the LLM
//! - **Tool coordination**: Register and execute tools asynchronously
//! - **Agent loop**: Orchestrate the conversation with tool execution
//! - **Context compaction**: Automatic context window management
//! - **Doom loop detection**: Prevent infinite tool call loops
//! - **Step control**: Programmatic agent execution control
//! - **Best-of-N**: Parallel generation with selection

pub mod agent_loop;
pub mod best_of_n;
pub mod compact_agent;
pub mod context;
pub mod conversation;
pub mod cost;
#[cfg(test)]
mod cost_tests;
pub mod doom_loop;
#[cfg(test)]
mod doom_loop_tests;
pub mod file_change_log;
pub mod guard;
pub mod logger;
pub mod orchestrator;
pub mod prompt;
pub mod rules;
pub mod session;
pub mod specialists;
#[cfg(test)]
mod specialists_tests;
pub mod state;
pub mod step_controller;
#[cfg(test)]
mod step_controller_tests;
pub mod subagent;
pub mod tool_coordinator;

#[cfg(test)]
mod tests;

// Re-exports
pub use crate::ipc::types::ApprovalDecision;
pub use agent_loop::{AgentConfig, AgentEvent, AgentLoop, AgentLoopError, AgentResult};
pub use best_of_n::{BestOfNConfig, BestOfNError, BestOfNExecutor, BestOfNResult, VariantResult};
pub use compact_agent::{
    CompactConversation, CompactionConfig, CompactionExt, CompactionResult, ContextManager,
    ContextStats,
};
pub use conversation::Conversation;
pub use doom_loop::{DoomLoopDetector, DoomLoopWarning, LoopStatistics, ToolCallPattern};
pub use file_change_log::{FileChangeLog, FileSnapshot};
pub use guard::CommandGuard;
pub use prompt::build_system_prompt;
pub use session::{create_agent_session, AgentSessionHandle, SessionConfig, SessionEvent};
pub use specialists::{AgentType, SPECIALIST_AGENT_TYPES};
pub use state::{
    is_valid_transition, AgentState, AgentStateTracker, StateTransitionReason, ACTIVITY_WINDOW,
    DEFAULT_IDLE_TIMEOUT,
};
pub use step_controller::{StepBuilder, StepControl, StepController};
pub use subagent::{SubAgentHandle, SubAgentManager, SubAgentStatus};
pub use tool_coordinator::{
    ToolCoordinator, ToolCoordinatorBuilder, ToolCoordinatorError, ToolExecutionResult, ToolHandler,
};
