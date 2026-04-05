//! Delivery state machine with transition validation and history tracking.

use std::time::Duration;

use chrono::Utc;
use tracing::debug;

use super::types::{DeliveryState, DeliveryStateTransition, LifecycleError, StateTrigger};

// ============================================================================
// TRANSITION VALIDATION
// ============================================================================

/// Validate whether a transition from `current` to `next` is legal.
fn is_valid_transition(current: &DeliveryState, next: &DeliveryState) -> bool {
    use DeliveryState::*;

    // Self-transitions are idempotent no-ops
    if current == next {
        return true;
    }

    match (current, next) {
        // Agent work states
        (Spawning, Working) => true,
        (Working, PrOpen | NeedsInput | Stuck | Errored | Done) => true,

        // PR pipeline
        (PrOpen, CiRunning | ReviewPending | ChangesRequested | Errored) => true,
        (CiRunning, CiPassed | CiFailed) => true,
        (CiFailed, Working | Errored) => true,
        (CiPassed, ReviewPending | Mergeable) => true,
        (ReviewPending, Approved | ChangesRequested) => true,
        (ChangesRequested, Working) => true,
        (Approved, Mergeable) => true,
        (Mergeable, Merged | Errored) => true,
        (Merged, Done) => true,

        // Exception recovery
        (NeedsInput, Working) => true,
        (Stuck, Working | Killed | Errored) => true,
        (Errored, Working | Terminated) => true,
        (Killed, Terminated) => true,

        // Terminal states accept no outgoing transitions
        _ => false,
    }
}

// ============================================================================
// STATE MACHINE
// ============================================================================

/// State machine tracking delivery lifecycle with full history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeliveryStateMachine {
    current: DeliveryState,
    history: Vec<DeliveryStateTransition>,
    entered_at: chrono::DateTime<chrono::Utc>,
    last_activity: chrono::DateTime<chrono::Utc>,
}

impl DeliveryStateMachine {
    /// Create a new state machine starting at `Spawning`.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            current: DeliveryState::Spawning,
            history: Vec::new(),
            entered_at: now,
            last_activity: now,
        }
    }

    /// Get the current delivery state.
    pub fn current_state(&self) -> &DeliveryState {
        &self.current
    }

    /// Attempt a state transition.
    ///
    /// Returns `Ok(())` on success, or a `LifecycleError` if the transition
    /// is invalid or the current state is terminal.
    pub fn transition(
        &mut self,
        to: DeliveryState,
        trigger: StateTrigger,
    ) -> Result<(), LifecycleError> {
        self.transition_with_metadata(to, trigger, serde_json::Value::Null)
    }

    /// Attempt a state transition with attached metadata.
    pub fn transition_with_metadata(
        &mut self,
        to: DeliveryState,
        trigger: StateTrigger,
        metadata: serde_json::Value,
    ) -> Result<(), LifecycleError> {
        if self.current.is_terminal() {
            return Err(LifecycleError::TerminalState(self.current));
        }

        if !self.can_transition_to(&to) {
            return Err(LifecycleError::InvalidTransition {
                from: self.current,
                to,
            });
        }

        let now = Utc::now();
        let transition_record = DeliveryStateTransition {
            from: self.current,
            to,
            triggered_by: trigger,
            timestamp: now,
            metadata,
        };

        debug!(
            from = %self.current,
            to = %to,
            trigger = ?trigger,
            "Delivery state transition"
        );

        self.current = to;
        self.entered_at = now;
        self.last_activity = now;
        self.history.push(transition_record);

        Ok(())
    }

    /// Check whether a transition to `target` is valid from the current state.
    pub fn can_transition_to(&self, target: &DeliveryState) -> bool {
        is_valid_transition(&self.current, target)
    }

    /// Get the full transition history.
    pub fn history(&self) -> &[DeliveryStateTransition] {
        &self.history
    }

    /// Duration spent in the current state.
    pub fn time_in_state(&self) -> Duration {
        let now = Utc::now();
        (now - self.entered_at).to_std().unwrap_or(Duration::ZERO)
    }

    /// Duration since the last activity (transition).
    pub fn time_since_activity(&self) -> Duration {
        let now = Utc::now();
        (now - self.last_activity)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }

    /// Update the last-activity timestamp without changing state.
    pub fn record_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

impl Default for DeliveryStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
