//! Agent Activity Detection
//!
//! Tracks agent activity states: active, idle, stuck, blocked, waiting_input.
//! Detects patterns that indicate the agent needs intervention.

#[cfg(test)]
pub mod tests;
pub mod tracker;
pub mod types;

pub use tracker::ActivityTracker;
pub use types::{ActivityConfig, ActivityState, BLOCKED_ERROR_THRESHOLD, TOOL_HISTORY_SIZE};
