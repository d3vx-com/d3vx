//! State machine transition tests (valid, invalid, terminal, history paths).

use super::machine::DeliveryStateMachine;
use super::types::{DeliveryState, LifecycleError, StateTrigger};

pub(crate) fn make_machine() -> DeliveryStateMachine {
    DeliveryStateMachine::new()
}

// -- Valid transitions ---------------------------------------------------

#[test]
fn test_spawning_to_working() {
    let mut sm = make_machine();
    assert!(sm
        .transition(DeliveryState::Working, StateTrigger::Auto)
        .is_ok());
    assert_eq!(*sm.current_state(), DeliveryState::Working);
}

#[test]
fn test_full_happy_path() {
    let mut sm = make_machine();

    let steps: Vec<(DeliveryState, StateTrigger)> = vec![
        (DeliveryState::Working, StateTrigger::Auto),
        (DeliveryState::PrOpen, StateTrigger::Auto),
        (DeliveryState::CiRunning, StateTrigger::CiEvent),
        (DeliveryState::CiPassed, StateTrigger::CiEvent),
        (DeliveryState::ReviewPending, StateTrigger::ReviewEvent),
        (DeliveryState::Approved, StateTrigger::ReviewEvent),
        (DeliveryState::Mergeable, StateTrigger::Auto),
        (DeliveryState::Merged, StateTrigger::Auto),
        (DeliveryState::Done, StateTrigger::Auto),
    ];

    for (state, trigger) in steps {
        assert!(
            sm.transition(state, trigger).is_ok(),
            "Failed to transition to {:?}",
            state
        );
    }
    assert_eq!(*sm.current_state(), DeliveryState::Done);
}

#[test]
fn test_ci_failure_retry_path() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::PrOpen, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::CiRunning, StateTrigger::CiEvent)
        .unwrap();
    sm.transition(DeliveryState::CiFailed, StateTrigger::CiEvent)
        .unwrap();

    // Retry after CI failure
    assert!(sm
        .transition(DeliveryState::Working, StateTrigger::Auto)
        .is_ok());
}

#[test]
fn test_changes_requested_path() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::PrOpen, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::CiRunning, StateTrigger::CiEvent)
        .unwrap();
    sm.transition(DeliveryState::CiPassed, StateTrigger::CiEvent)
        .unwrap();
    sm.transition(DeliveryState::ReviewPending, StateTrigger::ReviewEvent)
        .unwrap();
    sm.transition(DeliveryState::ChangesRequested, StateTrigger::ReviewEvent)
        .unwrap();

    // Go back to working to address feedback
    assert!(sm
        .transition(DeliveryState::Working, StateTrigger::Auto)
        .is_ok());
}

#[test]
fn test_stuck_to_killed() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Stuck, StateTrigger::Timeout)
        .unwrap();
    assert!(sm
        .transition(DeliveryState::Killed, StateTrigger::User)
        .is_ok());
    // Killed is not terminal; it must transition to Terminated
    assert!(!sm.current_state().is_terminal());
    assert!(sm
        .transition(DeliveryState::Terminated, StateTrigger::Auto)
        .is_ok());
    assert!(sm.current_state().is_terminal());
}

#[test]
fn test_errored_to_terminated() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Errored, StateTrigger::Auto)
        .unwrap();
    assert!(sm
        .transition(DeliveryState::Terminated, StateTrigger::Auto)
        .is_ok());
}

#[test]
fn test_errored_to_working_retry() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Errored, StateTrigger::Auto)
        .unwrap();
    assert!(sm
        .transition(DeliveryState::Working, StateTrigger::Auto)
        .is_ok());
}

#[test]
fn test_needs_input_to_working() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::NeedsInput, StateTrigger::Auto)
        .unwrap();
    assert!(sm
        .transition(DeliveryState::Working, StateTrigger::User)
        .is_ok());
}

// -- Invalid transitions -------------------------------------------------

#[test]
fn test_invalid_transition_spawning_to_done() {
    let mut sm = make_machine();
    let result = sm.transition(DeliveryState::Done, StateTrigger::Auto);
    assert!(result.is_err());
    match result.unwrap_err() {
        LifecycleError::InvalidTransition { from, to } => {
            assert_eq!(from, DeliveryState::Spawning);
            assert_eq!(to, DeliveryState::Done);
        }
        other => panic!("Expected InvalidTransition, got {:?}", other),
    }
}

#[test]
fn test_invalid_transition_working_to_merged() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    let result = sm.transition(DeliveryState::Merged, StateTrigger::Auto);
    assert!(result.is_err());
}

#[test]
fn test_invalid_transition_pr_open_to_approved() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::PrOpen, StateTrigger::Auto)
        .unwrap();
    // Must go through CI / review first
    let result = sm.transition(DeliveryState::Approved, StateTrigger::ReviewEvent);
    assert!(result.is_err());
}

#[test]
fn test_invalid_transition_ci_passed_to_working() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::PrOpen, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::CiRunning, StateTrigger::CiEvent)
        .unwrap();
    sm.transition(DeliveryState::CiPassed, StateTrigger::CiEvent)
        .unwrap();
    let result = sm.transition(DeliveryState::Working, StateTrigger::Auto);
    assert!(result.is_err());
}

// -- Terminal states reject further transitions ---------------------------

#[test]
fn test_terminal_done_rejects_transitions() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Done, StateTrigger::Auto)
        .unwrap();

    let result = sm.transition(DeliveryState::Working, StateTrigger::Auto);
    assert!(result.is_err());
    match result.unwrap_err() {
        LifecycleError::TerminalState(state) => {
            assert_eq!(state, DeliveryState::Done);
        }
        other => panic!("Expected TerminalState, got {:?}", other),
    }
}

#[test]
fn test_terminal_terminated_rejects_transitions() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Errored, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::Terminated, StateTrigger::Auto)
        .unwrap();

    let result = sm.transition(DeliveryState::Working, StateTrigger::Auto);
    assert!(result.is_err());
}

// -- History tracking ----------------------------------------------------

#[test]
fn test_history_tracks_all_transitions() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::PrOpen, StateTrigger::Auto)
        .unwrap();
    sm.transition(DeliveryState::CiRunning, StateTrigger::CiEvent)
        .unwrap();

    let history = sm.history();
    assert_eq!(history.len(), 3);

    assert_eq!(history[0].from, DeliveryState::Spawning);
    assert_eq!(history[0].to, DeliveryState::Working);
    assert_eq!(history[0].triggered_by, StateTrigger::Auto);

    assert_eq!(history[1].from, DeliveryState::Working);
    assert_eq!(history[1].to, DeliveryState::PrOpen);

    assert_eq!(history[2].from, DeliveryState::PrOpen);
    assert_eq!(history[2].to, DeliveryState::CiRunning);
    assert_eq!(history[2].triggered_by, StateTrigger::CiEvent);
}

#[test]
fn test_history_includes_metadata() {
    let mut sm = make_machine();
    let meta = serde_json::json!({ "ci_url": "https://ci.example.com/123" });
    sm.transition_with_metadata(DeliveryState::Working, StateTrigger::CiEvent, meta.clone())
        .unwrap();

    let history = sm.history();
    assert_eq!(history[0].metadata, meta);
}

#[test]
fn test_history_empty_on_new_machine() {
    let sm = make_machine();
    assert!(sm.history().is_empty());
}
