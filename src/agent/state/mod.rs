//! Agent State Machine
//!
//! Manages granular agent states for activity detection and autonomous decision-making.
//! Supports 5 states: Thinking, ToolExecution, Idle, WaitingInput, Done.

mod tests;
mod tests_tracker;
mod tracker;
mod types;

// Re-export all public types
pub use tracker::AgentStateTracker;
pub use types::{
    is_valid_transition, AgentState, StateTransitionReason, ACTIVITY_WINDOW, DEFAULT_IDLE_TIMEOUT,
};
