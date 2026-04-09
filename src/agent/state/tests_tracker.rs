//! Agent state tracker tests

use crate::agent::state::{AgentState, AgentStateTracker, StateTransitionReason};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_state_tracker_initial_state() {
    let tracker = AgentStateTracker::new();
    assert_eq!(tracker.current_state().await, AgentState::Idle);
}

#[tokio::test]
async fn test_state_tracker_activate() {
    let tracker = AgentStateTracker::new();
    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::Thinking);
}

#[tokio::test]
async fn test_state_tracker_invalid_transition() {
    let tracker = AgentStateTracker::new();
    assert!(!tracker.request_input("test").await);
    assert_eq!(tracker.current_state().await, AgentState::Idle);
}

#[tokio::test]
async fn test_state_tracker_request_input_flow() {
    let tracker = AgentStateTracker::new();

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::Thinking);

    assert!(tracker.request_input("AskUser").await);
    assert_eq!(tracker.current_state().await, AgentState::WaitingInput);

    assert!(tracker.receive_input().await);
    assert_eq!(tracker.current_state().await, AgentState::Thinking);
}

#[tokio::test]
async fn test_state_tracker_activity_flow() {
    let tracker = AgentStateTracker::new();

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::Thinking);

    assert!(
        tracker
            .transition_to(
                AgentState::ToolExecution,
                StateTransitionReason::ActivityDetected
            )
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::ToolExecution);

    assert!(tracker.complete().await);
    assert_eq!(tracker.current_state().await, AgentState::Done);
}

#[tokio::test]
async fn test_state_tracker_complete() {
    let tracker = AgentStateTracker::new();

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert!(tracker.complete().await);
    assert_eq!(tracker.current_state().await, AgentState::Done);

    assert!(
        !tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::Done);
}

#[tokio::test]
async fn test_state_tracker_fail() {
    let tracker = AgentStateTracker::new();

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert!(tracker.fail("Something went wrong").await);
    assert_eq!(tracker.current_state().await, AgentState::Done);
}

#[tokio::test]
async fn test_state_tracker_reset() {
    let tracker = AgentStateTracker::new();

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert!(tracker.reset().await);
    assert_eq!(tracker.current_state().await, AgentState::Idle);

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert!(tracker.complete().await);
    assert!(!tracker.reset().await);
    assert_eq!(tracker.current_state().await, AgentState::Done);
}

#[tokio::test]
async fn test_state_tracker_idle_timeout() {
    let tracker = AgentStateTracker::with_idle_timeout(Duration::from_millis(50));

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(tracker.current_state().await, AgentState::Thinking);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = tracker.check_idle_timeout().await;
    assert_eq!(result, Some(AgentState::Idle));
    assert_eq!(tracker.current_state().await, AgentState::Idle);
}

#[tokio::test]
async fn test_state_tracker_record_activity() {
    let tracker = AgentStateTracker::with_idle_timeout(Duration::from_millis(50));

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );

    tokio::time::sleep(Duration::from_millis(30)).await;
    tracker.record_activity().await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    let result = tracker.check_idle_timeout().await;
    assert!(result.is_none());
    assert_eq!(tracker.current_state().await, AgentState::Thinking);
}

#[tokio::test]
async fn test_state_tracker_callback() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    let callback = Arc::new(
        move |_from: AgentState, _to: AgentState, _reason: &StateTransitionReason| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    let tracker = AgentStateTracker::new().with_state_change_callback(callback);

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    assert!(tracker.complete().await);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_time_until_idle() {
    let tracker = AgentStateTracker::with_idle_timeout(Duration::from_secs(60));

    assert!(tracker.time_until_idle().await.is_none());

    assert!(
        tracker
            .activate(StateTransitionReason::ActivityDetected)
            .await
    );
    let remaining = tracker.time_until_idle().await;
    assert!(remaining.is_some());
    let remaining = remaining.unwrap();
    assert!(remaining <= Duration::from_secs(60));
    assert!(remaining > Duration::from_secs(58));
}
