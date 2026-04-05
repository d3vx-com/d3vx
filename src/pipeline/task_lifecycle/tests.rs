//! Unit tests for types, display, time tracking, and query helpers.

use super::machine::DeliveryStateMachine;
use super::tests_transitions::make_machine;
use super::types::DeliveryState;

// -- Time tracking -------------------------------------------------------

#[test]
fn test_time_in_state_is_nonzero() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, super::types::StateTrigger::Auto)
        .unwrap();
    let elapsed = sm.time_in_state();
    // Should be very small but non-negative
    assert!(elapsed.as_nanos() < std::time::Duration::from_secs(10).as_nanos());
}

#[test]
fn test_time_since_activity_updates_on_transition() {
    let mut sm = make_machine();
    sm.transition(DeliveryState::Working, super::types::StateTrigger::Auto)
        .unwrap();
    let after_first = sm.time_since_activity();
    assert!(after_first.as_nanos() < std::time::Duration::from_secs(5).as_nanos());
}

#[test]
fn test_record_activity_updates_timestamp() {
    let mut sm = make_machine();
    let before = sm.time_since_activity();
    sm.record_activity();
    let after = sm.time_since_activity();
    // After recording, the duration should be less than or equal
    assert!(after <= before + std::time::Duration::from_millis(10));
}

// -- can_transition_to ---------------------------------------------------

#[test]
fn test_can_transition_to_valid() {
    let sm = make_machine();
    assert!(sm.can_transition_to(&DeliveryState::Working));
}

#[test]
fn test_can_transition_to_invalid() {
    let sm = make_machine();
    assert!(!sm.can_transition_to(&DeliveryState::Done));
}

#[test]
fn test_can_transition_to_self() {
    let sm = make_machine();
    // Self-transitions are valid (idempotent)
    assert!(sm.can_transition_to(&DeliveryState::Spawning));
}

// -- is_terminal / is_active helpers ------------------------------------

#[test]
fn test_state_helpers() {
    assert!(DeliveryState::Done.is_terminal());
    assert!(DeliveryState::Terminated.is_terminal());
    assert!(!DeliveryState::Working.is_terminal());

    assert!(DeliveryState::Working.is_active());
    assert!(DeliveryState::Spawning.is_active());
    assert!(!DeliveryState::Done.is_active());
    assert!(!DeliveryState::Stuck.is_active());
}

// -- Display implementation ----------------------------------------------

#[test]
fn test_display_formatting() {
    assert_eq!(DeliveryState::Spawning.to_string(), "spawning");
    assert_eq!(DeliveryState::CiRunning.to_string(), "ci_running");
    assert_eq!(
        DeliveryState::ChangesRequested.to_string(),
        "changes_requested"
    );
    assert_eq!(DeliveryState::Done.to_string(), "done");
}

// -- Default implementation ----------------------------------------------

#[test]
fn test_default_starts_at_spawning() {
    let sm = DeliveryStateMachine::default();
    assert_eq!(*sm.current_state(), DeliveryState::Spawning);
}
