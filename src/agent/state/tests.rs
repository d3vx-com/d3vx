//! Agent state type tests

use super::{is_valid_transition, AgentState, StateTransitionReason};

#[test]
fn test_agent_state_display() {
    assert_eq!(AgentState::Thinking.to_string(), "Thinking");
    assert_eq!(AgentState::ToolExecution.to_string(), "ToolExecution");
    assert_eq!(AgentState::Idle.to_string(), "Idle");
    assert_eq!(AgentState::WaitingInput.to_string(), "WaitingInput");
    assert_eq!(AgentState::Done.to_string(), "Done");
}

#[test]
fn test_valid_transitions_from_thinking() {
    assert!(is_valid_transition(AgentState::Thinking, AgentState::Idle));
    assert!(is_valid_transition(
        AgentState::Thinking,
        AgentState::WaitingInput
    ));
    assert!(is_valid_transition(
        AgentState::Thinking,
        AgentState::ToolExecution
    ));
    assert!(is_valid_transition(AgentState::Thinking, AgentState::Done));
    assert!(is_valid_transition(
        AgentState::Thinking,
        AgentState::Thinking
    ));
}

#[test]
fn test_valid_transitions_from_idle() {
    assert!(is_valid_transition(AgentState::Idle, AgentState::Thinking));
    assert!(is_valid_transition(AgentState::Idle, AgentState::Done));
    assert!(!is_valid_transition(AgentState::Idle, AgentState::Idle));
    assert!(!is_valid_transition(
        AgentState::Idle,
        AgentState::WaitingInput
    ));
}

#[test]
fn test_valid_transitions_from_waiting_input() {
    assert!(is_valid_transition(
        AgentState::WaitingInput,
        AgentState::Thinking
    ));
    assert!(is_valid_transition(
        AgentState::WaitingInput,
        AgentState::Done
    ));
    assert!(!is_valid_transition(
        AgentState::WaitingInput,
        AgentState::Idle
    ));
}

#[test]
fn test_valid_transitions_from_done() {
    assert!(!is_valid_transition(AgentState::Done, AgentState::Thinking));
    assert!(!is_valid_transition(AgentState::Done, AgentState::Idle));
    assert!(!is_valid_transition(
        AgentState::Done,
        AgentState::WaitingInput
    ));
    assert!(!is_valid_transition(AgentState::Done, AgentState::Done));
}

#[test]
fn test_state_transition_reason_display() {
    assert_eq!(
        StateTransitionReason::ActivityDetected.to_string(),
        "Activity detected"
    );
    assert_eq!(
        StateTransitionReason::ToolRequiresInput {
            tool_name: "AskUser".to_string()
        }
        .to_string(),
        "Tool 'AskUser' requires input"
    );
    assert_eq!(
        StateTransitionReason::RetryableError {
            error: "Network error".to_string()
        }
        .to_string(),
        "Retryable error: Network error"
    );
}
