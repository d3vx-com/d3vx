//! Session State Persister
//!
//! Maps AgentState to SessionState and persists to SQLite on every transition.
//! This bridges the agent loop's internal state with the store's 15-state lifecycle.

use tracing::{debug, warn};

use crate::agent::state::types::StateTransitionReason;
use crate::agent::state::AgentState;
use crate::store::session::SessionState;

/// Maps an AgentState to the corresponding SessionState.
///
/// AgentState has 5 states; SessionState has 15. This mapping provides
/// the most semantically correct translation.
pub fn agent_to_session_state(state: &AgentState) -> SessionState {
    match state {
        AgentState::Thinking => SessionState::Running,
        AgentState::ToolExecution => SessionState::Running,
        AgentState::Idle => SessionState::Idle,
        AgentState::WaitingInput => SessionState::WaitingInput,
        AgentState::Done => SessionState::Stopped,
    }
}

/// Persists session state to SQLite on every agent state change.
///
/// This is called as a callback from the AgentStateTracker whenever the
/// agent transitions between states.
pub struct SessionStatePersister;

impl SessionStatePersister {
    /// Create a state change callback that persists to the database.
    ///
    /// Returns a closure suitable for `with_state_change_callback`.
    pub fn callback(
        session_id: String,
        db: crate::store::database::DatabaseHandle,
    ) -> impl Fn(AgentState, AgentState, &StateTransitionReason) + Send + Sync + 'static {
        move |old_state, new_state, _reason| {
            let session_id = session_id.clone();
            let db = db.clone();
            let session_state = agent_to_session_state(&new_state);

            // Spawn async to avoid blocking the agent loop
            tokio::spawn(async move {
                if let Err(e) = persist_state(&session_id, &session_state, &db).await {
                    warn!(
                        session_id,
                        error = %e,
                        "Failed to persist session state"
                    );
                } else {
                    debug!(
                        session_id,
                        from = ?old_state,
                        to = ?new_state,
                        session_state = ?session_state,
                        "Session state persisted"
                    );
                }
            });
        }
    }
}

/// Persist the session state to SQLite.
async fn persist_state(
    session_id: &str,
    state: &SessionState,
    db: &crate::store::database::DatabaseHandle,
) -> anyhow::Result<()> {
    let db = db.lock();
    let store = crate::store::session::SessionStore::from_connection(db.connection());

    let update = crate::store::session::SessionUpdate {
        state: Some(state.clone()),
        ..Default::default()
    };

    store.update(session_id, update)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_to_session_state_thinking() {
        assert_eq!(
            agent_to_session_state(&AgentState::Thinking),
            SessionState::Running
        );
    }

    #[test]
    fn test_agent_to_session_state_tool_execution() {
        assert_eq!(
            agent_to_session_state(&AgentState::ToolExecution),
            SessionState::Running
        );
    }

    #[test]
    fn test_agent_to_session_state_idle() {
        assert_eq!(
            agent_to_session_state(&AgentState::Idle),
            SessionState::Idle
        );
    }

    #[test]
    fn test_agent_to_session_state_waiting_input() {
        assert_eq!(
            agent_to_session_state(&AgentState::WaitingInput),
            SessionState::WaitingInput
        );
    }

    #[test]
    fn test_agent_to_session_state_done() {
        assert_eq!(
            agent_to_session_state(&AgentState::Done),
            SessionState::Stopped
        );
    }
}
