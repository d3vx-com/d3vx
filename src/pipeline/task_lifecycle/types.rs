//! Delivery state types, trigger enum, and transition record.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// DELIVERY STATE
// ============================================================================

/// Extended delivery states for the autonomous delivery pipeline.
///
/// Covers agent work, PR lifecycle, CI results, code review, and terminal states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    // Agent work states
    Spawning,
    Working,

    // PR pipeline states
    PrOpen,
    CiRunning,
    CiPassed,
    CiFailed,
    ReviewPending,
    ChangesRequested,
    Approved,
    Mergeable,
    Merged,

    // Exception states
    NeedsInput,
    Stuck,
    Errored,
    Killed,

    // Terminal
    Done,
    Terminated,
}

impl DeliveryState {
    /// Check whether this state is terminal (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, DeliveryState::Done | DeliveryState::Terminated)
    }

    /// Check whether this state represents active agent work.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            DeliveryState::Spawning
                | DeliveryState::Working
                | DeliveryState::PrOpen
                | DeliveryState::CiRunning
                | DeliveryState::ReviewPending
                | DeliveryState::ChangesRequested
                | DeliveryState::Approved
                | DeliveryState::Mergeable
                | DeliveryState::Merged
                | DeliveryState::NeedsInput
        )
    }
}

impl std::fmt::Display for DeliveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeliveryState::Spawning => write!(f, "spawning"),
            DeliveryState::Working => write!(f, "working"),
            DeliveryState::PrOpen => write!(f, "pr_open"),
            DeliveryState::CiRunning => write!(f, "ci_running"),
            DeliveryState::CiPassed => write!(f, "ci_passed"),
            DeliveryState::CiFailed => write!(f, "ci_failed"),
            DeliveryState::ReviewPending => write!(f, "review_pending"),
            DeliveryState::ChangesRequested => write!(f, "changes_requested"),
            DeliveryState::Approved => write!(f, "approved"),
            DeliveryState::Mergeable => write!(f, "mergeable"),
            DeliveryState::Merged => write!(f, "merged"),
            DeliveryState::NeedsInput => write!(f, "needs_input"),
            DeliveryState::Stuck => write!(f, "stuck"),
            DeliveryState::Errored => write!(f, "errored"),
            DeliveryState::Killed => write!(f, "killed"),
            DeliveryState::Done => write!(f, "done"),
            DeliveryState::Terminated => write!(f, "terminated"),
        }
    }
}

// ============================================================================
// STATE TRIGGER
// ============================================================================

/// What caused a state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateTrigger {
    /// Automatic transition (e.g. pipeline progression)
    Auto,
    /// User-initiated action
    User,
    /// CI webhook result
    CiEvent,
    /// PR review event
    ReviewEvent,
    /// Timeout expired
    Timeout,
}

// ============================================================================
// TRANSITION RECORD
// ============================================================================

/// Record of a single state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStateTransition {
    /// State before the transition
    pub from: DeliveryState,
    /// State after the transition
    pub to: DeliveryState,
    /// What triggered this transition
    pub triggered_by: StateTrigger,
    /// When the transition occurred
    pub timestamp: DateTime<Utc>,
    /// Optional metadata (e.g. CI run URL, reviewer name)
    pub metadata: serde_json::Value,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors from the delivery lifecycle state machine.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    /// The requested transition is not valid from the current state.
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: DeliveryState,
        to: DeliveryState,
    },

    /// The current state is terminal and cannot transition further.
    #[error("State is terminal: {0:?}")]
    TerminalState(DeliveryState),
}
