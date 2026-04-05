//! Agent Session State Machine
//!
//! Validates directed-graph progressions for the 15 granular Agent session tracking states.

use crate::store::session::{Session, SessionState};
use thiserror::Error;

/// Errors thrown by the state machine
#[derive(Debug, Error)]
pub enum StateError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: SessionState,
        to: SessionState,
    },
}

/// Applies a validated state transition directly onto the target session
pub fn transition(session: &mut Session, new_state: SessionState) -> Result<(), StateError> {
    if !is_valid_transition(&session.state, &new_state) {
        return Err(StateError::InvalidTransition {
            from: session.state.clone(),
            to: new_state,
        });
    }

    session.state = new_state;
    Ok(())
}

/// The core transition logic validating authorized Graph moves
fn is_valid_transition(current: &SessionState, next: &SessionState) -> bool {
    use SessionState::*;

    match (current, next) {
        // Initialization
        (Spawning, Initializing) => true,
        (Initializing, Running) => true,

        // Active work loops
        (Running, Idle) => true,
        (Idle, Running) => true,

        (Running, WaitingInput) => true,
        (WaitingInput, Running) => true,

        (Running, Blocked) => true,
        (Blocked, Running) => true,

        // Halting and Terminations
        (Running, Stopping) => true,
        (Stopping, Stopped) => true,

        (Running, Crashed) => true,
        (Running, Failed) => true,

        // Integration execution
        (Running, Merging) => true,
        (Merging, Merged) => true,
        (Running, Abandoned) => true,

        // Cleanup
        (Stopped | Crashed | Failed | Merged | Abandoned, Cleaning) => true,
        (Cleaning, Cleaned) => true,

        // Self-transitions (idempotent state updates)
        _ if current == next => true,

        // Any unspecified progression is illegal
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut s = Session {
            id: "s-1".into(),
            task_id: None,
            provider: "claude".into(),
            model: "claude".into(),
            messages: "[]".into(),
            token_count: 0,
            summary: None,
            project_path: None,
            parent_session_id: None,
            created_at: "".into(),
            updated_at: "".into(),
            metadata: "{}".into(),
            state: SessionState::Spawning,
        };

        // Complete normal lifecycle
        assert!(transition(&mut s, SessionState::Initializing).is_ok());
        assert!(transition(&mut s, SessionState::Running).is_ok());

        // Loop interactions
        assert!(transition(&mut s, SessionState::Idle).is_ok());
        assert!(transition(&mut s, SessionState::Running).is_ok());
        assert!(transition(&mut s, SessionState::WaitingInput).is_ok());
        assert!(transition(&mut s, SessionState::Running).is_ok());

        // Terminal shutdown
        assert!(transition(&mut s, SessionState::Stopping).is_ok());
        assert!(transition(&mut s, SessionState::Stopped).is_ok());
        assert!(transition(&mut s, SessionState::Cleaning).is_ok());
        assert!(transition(&mut s, SessionState::Cleaned).is_ok());
    }

    #[test]
    fn test_invalid_transitions() {
        let mut s = Session {
            id: "s-1".into(),
            task_id: None,
            provider: "claude".into(),
            model: "claude".into(),
            messages: "[]".into(),
            token_count: 0,
            summary: None,
            project_path: None,
            parent_session_id: None,
            created_at: "".into(),
            updated_at: "".into(),
            metadata: "{}".into(),
            state: SessionState::Spawning,
        };

        // Can't jump straight from Spawning to Running
        assert!(transition(&mut s, SessionState::Running).is_err());

        // Can't Merge from Idle
        s.state = SessionState::Idle;
        assert!(transition(&mut s, SessionState::Merging).is_err());

        // Can't jump to Cleaned before Cleaning
        s.state = SessionState::Stopped;
        assert!(transition(&mut s, SessionState::Cleaned).is_err());
    }
}
