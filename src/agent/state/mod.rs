//! Agent State Machine
//!
//! Manages granular agent states for activity detection and autonomous decision-making.
//! Supports 5 states: Thinking, ToolExecution, Idle, WaitingInput, Done.

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_tracker;
mod tracker;
pub mod types;

// Re-export all public types
pub use tracker::AgentStateTracker;
pub use types::{
    is_valid_transition, AgentState, StateTransitionReason, ACTIVITY_WINDOW, DEFAULT_IDLE_TIMEOUT,
};
