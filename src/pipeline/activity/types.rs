//! Activity Detection Types
//!
//! Data structures and configuration for activity tracking.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Activity state of an agent or task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    /// Agent is actively processing (tool calls, generation)
    Active,
    /// Agent is ready but idle (waiting for new work)
    Ready,
    /// Agent has been idle for too long
    Idle,
    /// Agent is waiting for user permission/input
    WaitingInput,
    /// Agent is blocked by an error or external dependency
    Blocked,
    /// Agent appears stuck in a loop (repeating patterns)
    Stuck,
    /// Agent process has exited
    Exited,
}

impl std::fmt::Display for ActivityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityState::Active => write!(f, "active"),
            ActivityState::Ready => write!(f, "ready"),
            ActivityState::Idle => write!(f, "idle"),
            ActivityState::WaitingInput => write!(f, "waiting_input"),
            ActivityState::Blocked => write!(f, "blocked"),
            ActivityState::Stuck => write!(f, "stuck"),
            ActivityState::Exited => write!(f, "exited"),
        }
    }
}

/// Maximum number of tool calls retained for stuck detection
pub const TOOL_HISTORY_SIZE: usize = 20;

/// Number of consecutive errors before declaring blocked
pub const BLOCKED_ERROR_THRESHOLD: usize = 5;

/// Configuration for activity detection thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityConfig {
    /// Time before considering agent idle (default: 120s)
    pub idle_threshold: Duration,
    /// Time before considering agent stuck (default: 300s)
    pub stuck_threshold: Duration,
    /// Number of repeated patterns before declaring stuck (default: 3)
    pub stuck_repeat_threshold: usize,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        Self {
            idle_threshold: Duration::from_secs(120),
            stuck_threshold: Duration::from_secs(300),
            stuck_repeat_threshold: 3,
        }
    }
}
