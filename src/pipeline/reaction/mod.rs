//! Reaction Engine for Autonomous CI/Review Handling
//!
//! This module provides automatic responses to external events (CI failures,
//! review comments, merge conflicts, stuck agents) without human intervention.
//!
//! # Architecture
//!
//! ```text
//! External Events (CI, Reviews, Conflicts, Heartbeats)
//!         |
//!         v
//! +-------------------+
//! |  ReactionEvent    |  <-- Normalized event representation
//! +-------------------+
//!         |
//!         v
//! +-------------------+
//! |  ReactionEngine   |  <-- Rules evaluation + action dispatch
//! +-------------------+
//!         |
//!    +----+----+----+
//!    |    |    |    |
//!   AutoFix Notify Escalate Checkpoint
//! ```

mod actions;
mod audit;
mod config;
mod conversion;
mod engine;
mod handlers;
mod notification;
mod stuck_detection;
mod types;

// Re-export all public types
pub use actions::{
    AgentNudge, AgentNudgeResult, AgentRestart, EscalationAction, EscalationPolicy,
    EscalationStatus, EscalationTracker, NudgeComposer, NudgePriority, RestartPlanner,
    RestartResult, RestartStrategy,
};
pub use config::*;
pub use engine::ReactionEngine;
pub use notification::*;
pub use types::*;

#[cfg(test)]
mod tests;
