//! Session tracker: state machine owning the lifecycle of one session.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tracing::debug;

use super::types::{
    PhaseMetadata, PhaseTransition, SessionPhase, SessionSummary, TransitionCause, TransitionError,
};

// ============================================================================
// VALID TRANSITIONS MAP
// ============================================================================

lazy_static::lazy_static! {
    pub(crate) static ref TRANSITIONS: HashMap<SessionPhase, Vec<SessionPhase>> = {
        use SessionPhase::*;
        let mut m = HashMap::new();
        m.insert(Provisioning,       vec![Initializing, Cancelled]);
        m.insert(Initializing,       vec![Working, Crashed, Cancelled]);
        m.insert(Working,            vec![PrOpen, AwaitingApproval, Done, Stuck, NeedsInput, Cancelled, TimedOut]);
        m.insert(AwaitingApproval,   vec![Approved, Cancelled, TimedOut]);
        m.insert(Approved,           vec![Working, Cancelled]);
        m.insert(PrOpen,             vec![CiRunning, ReviewPending, Merged, Stuck, Cancelled]);
        m.insert(CiRunning,          vec![CiPassed, CiFailed, TimedOut]);
        m.insert(CiPassed,           vec![ReviewPending]);
        m.insert(CiFailed,           vec![Working, Stuck, Cancelled]);
        m.insert(ReviewPending,      vec![ChangesRequested, ApprovedForMerge, Stuck, TimedOut]);
        m.insert(ChangesRequested,   vec![Working, Cancelled]);
        m.insert(ApprovedForMerge,   vec![Merging, Stuck, Cancelled]);
        m.insert(Merging,            vec![Merged, Crashed, Stuck]);
        m.insert(Stuck,              vec![Working, Crashed, Cancelled, Orphaned]);
        m.insert(NeedsInput,         vec![Working, Cancelled]);
        m.insert(Crashed,            vec![Provisioning, Cancelled, Orphaned]);
        m.insert(Cancelled,          vec![]);
        m.insert(TimedOut,           vec![Working, Cancelled, Orphaned]);
        m.insert(Merged,             vec![]);
        m.insert(Done,               vec![]);
        m.insert(Orphaned,           vec![Provisioning, Cancelled]);
        m
    };
}

// ============================================================================
// SESSION TRACKER
// ============================================================================

/// Owns the state machine for one session's lifecycle.
///
/// Tracks the current phase, validates transitions against the legal graph,
/// records history, and measures time spent in each phase.
#[derive(Debug)]
pub struct SessionTracker {
    session_id: String,
    current: SessionPhase,
    metadata: PhaseMetadata,
    history: Vec<PhaseTransition>,
    phase_entered: Instant,
    /// Epoch-millis timestamp when the tracker was created.
    created_at_ms: u64,
}

impl SessionTracker {
    /// Create a new tracker starting at `Provisioning`.
    pub fn new(session_id: String) -> Self {
        let now_ms = epoch_millis();
        Self {
            session_id,
            current: SessionPhase::Provisioning,
            metadata: PhaseMetadata {
                entered_at: now_ms,
                transition_count: 0,
                last_probe_result: None,
                custom_data: HashMap::new(),
            },
            history: Vec::new(),
            phase_entered: Instant::now(),
            created_at_ms: now_ms,
        }
    }

    /// Create a tracker that starts at an arbitrary phase (for recovery).
    pub fn with_phase(session_id: String, phase: SessionPhase) -> Self {
        let now_ms = epoch_millis();
        Self {
            session_id,
            current: phase,
            metadata: PhaseMetadata {
                entered_at: now_ms,
                transition_count: 0,
                last_probe_result: None,
                custom_data: HashMap::new(),
            },
            history: Vec::new(),
            phase_entered: Instant::now(),
            created_at_ms: now_ms,
        }
    }

    /// The session identifier this tracker manages.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Current phase of the session.
    pub fn current_phase(&self) -> &SessionPhase {
        &self.current
    }

    /// Full transition history.
    pub fn history(&self) -> &[PhaseTransition] {
        &self.history
    }

    /// Metadata attached to the current phase.
    pub fn metadata(&self) -> &PhaseMetadata {
        &self.metadata
    }

    /// Mutable access to metadata for external updates.
    pub fn metadata_mut(&mut self) -> &mut PhaseMetadata {
        &mut self.metadata
    }

    /// Duration spent in the current phase so far.
    pub fn time_in_phase(&self) -> Duration {
        self.phase_entered.elapsed()
    }

    /// Whether the session is in a terminal phase.
    pub fn is_terminal(&self) -> bool {
        self.current.is_terminal()
    }

    /// Whether the session is in an actionable phase requiring human attention.
    pub fn is_actionable(&self) -> bool {
        self.current.is_actionable()
    }

    /// Check whether a transition to `target` is valid from the current phase.
    pub fn can_transition_to(&self, target: &SessionPhase) -> bool {
        valid_transition(&self.current, target)
    }

    /// Attempt a transition to `target` with the given `cause`.
    ///
    /// Returns `Ok(())` on success, or a `TransitionError` if the transition
    /// is invalid or the session is already terminal.
    pub fn transition_to(
        &mut self,
        target: SessionPhase,
        cause: TransitionCause,
    ) -> Result<(), TransitionError> {
        if self.current.is_terminal() {
            return Err(TransitionError::AlreadyTerminal(self.current));
        }

        if !valid_transition(&self.current, &target) {
            return Err(TransitionError::InvalidTransition {
                from: self.current,
                to: target,
            });
        }

        debug!(
            session_id = %self.session_id,
            from = %self.current,
            to = %target,
            cause = ?cause,
            "Session phase transition"
        );

        let now_ms = epoch_millis();
        let record = PhaseTransition {
            from: self.current,
            to: target,
            trigger: cause,
            timestamp: now_ms,
        };

        self.current = target;
        self.metadata.entered_at = now_ms;
        self.metadata.transition_count += 1;
        self.phase_entered = Instant::now();
        self.history.push(record);

        Ok(())
    }

    /// Produce a lightweight summary of the session state.
    pub fn summarize(&self) -> SessionSummary {
        let duration_secs = epoch_millis().saturating_sub(self.created_at_ms) as f64 / 1000.0;
        SessionSummary {
            session_id: self.session_id.clone(),
            phase: self.current,
            duration_secs,
            pr_url: self.metadata.custom_data.get("pr_url").cloned(),
            branch: self.metadata.custom_data.get("branch").cloned(),
            cost_usd: self
                .metadata
                .custom_data
                .get("cost_usd")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0),
        }
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Check whether a transition from `from` to `to` is legal.
fn valid_transition(from: &SessionPhase, to: &SessionPhase) -> bool {
    if from == to {
        return true;
    }
    match TRANSITIONS.get(from) {
        Some(allowed) => allowed.contains(to),
        None => false,
    }
}

/// Current epoch time in milliseconds.
fn epoch_millis() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}
