//! Session lifecycle types: phase enum, transition record, metadata, and summary.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// SESSION PHASE
// ============================================================================

/// Phases tracked throughout a session's lifecycle.
///
/// Covers provisioning, agent work, PR pipeline, CI, review, merge, and
/// exception states. Terminal phases (Done, Merged, Cancelled) accept no
/// further transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    // Provisioning
    Provisioning,
    Initializing,

    // Agent work
    Working,
    AwaitingApproval,
    Approved,

    // PR pipeline
    PrOpen,
    CiRunning,
    CiPassed,
    CiFailed,

    // Review
    ReviewPending,
    ChangesRequested,
    ApprovedForMerge,

    // Merge
    Merging,
    Merged,

    // Success
    Done,

    // Exception
    Stuck,
    NeedsInput,
    Crashed,
    Cancelled,
    TimedOut,
    Orphaned,
}

impl SessionPhase {
    /// Terminal phases accept no outgoing transitions.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SessionPhase::Done | SessionPhase::Merged | SessionPhase::Cancelled
        )
    }

    /// Actionable phases indicate a human should intervene.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            SessionPhase::AwaitingApproval
                | SessionPhase::NeedsInput
                | SessionPhase::ReviewPending
                | SessionPhase::ChangesRequested
                | SessionPhase::Stuck
                | SessionPhase::CiFailed
        )
    }
}

impl std::fmt::Display for SessionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_else(|_| "\"unknown\"".to_string());
        // Strip the surrounding quotes from the JSON string.
        write!(f, "{}", s.trim_matches('"'))
    }
}

// ============================================================================
// TRANSITION CAUSE
// ============================================================================

/// What caused a phase transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionCause {
    AutoDetected,
    PollResult,
    ExternalEvent,
    UserAction,
    SystemAction,
    TimeoutExpired,
}

// ============================================================================
// TRANSITION RECORD
// ============================================================================

/// Record of a single phase transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseTransition {
    /// Phase before the transition.
    pub from: SessionPhase,
    /// Phase after the transition.
    pub to: SessionPhase,
    /// What triggered the transition.
    pub trigger: TransitionCause,
    /// Epoch-millis timestamp when the transition occurred.
    pub timestamp: u64,
}

// ============================================================================
// PHASE METADATA
// ============================================================================

/// Metadata attached to the current phase of a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseMetadata {
    /// Epoch-millis when the current phase was entered.
    pub entered_at: u64,
    /// Number of transitions this session has undergone.
    pub transition_count: usize,
    /// Most recent probe result, if any.
    pub last_probe_result: Option<String>,
    /// Arbitrary key-value data attached by consumers.
    pub custom_data: HashMap<String, String>,
}

impl Default for PhaseMetadata {
    fn default() -> Self {
        Self {
            entered_at: 0,
            transition_count: 0,
            last_probe_result: None,
            custom_data: HashMap::new(),
        }
    }
}

// ============================================================================
// SESSION SUMMARY
// ============================================================================

/// Lightweight summary of a session's current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub phase: SessionPhase,
    pub duration_secs: f64,
    pub pr_url: Option<String>,
    pub branch: Option<String>,
    pub cost_usd: f64,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors produced by the session tracker.
#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    /// The requested transition is not valid from the current phase.
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: SessionPhase,
        to: SessionPhase,
    },

    /// The current phase is terminal and cannot transition further.
    #[error("Session is already in terminal phase: {0:?}")]
    AlreadyTerminal(SessionPhase),

    /// An unrecognized or unsupported phase was encountered.
    #[error("Unknown phase encountered")]
    UnknownPhase,
}
