//! Task state machine tests

use super::state_machine::TaskState;

// ── Display Tests ─────────────────────────────────────────────

#[test]
fn test_task_state_display_all_states() {
    for state in TaskState::all() {
        let s = state.to_string();
        assert!(!s.is_empty(), "Empty display for {:?}", state);
        assert_eq!(
            s,
            s.to_uppercase(),
            "{:?} display should be SCREAMING_SNAKE_CASE",
            state
        );
    }
}

#[test]
fn test_task_state_display_spot_checks() {
    assert_eq!(TaskState::Backlog.to_string(), "BACKLOG");
    assert_eq!(TaskState::Done.to_string(), "DONE");
    assert_eq!(TaskState::Failed.to_string(), "FAILED");
}

// ── FromStr Tests ─────────────────────────────────────────────

#[test]
fn test_from_str_roundtrip_all_states() {
    for state in TaskState::all() {
        let from_str: Result<TaskState, _> = state.to_string().parse();
        assert_eq!(from_str.unwrap(), *state, "FromStr roundtrip failed for {:?}", state);
    }
}

#[test]
fn test_from_str_case_insensitive() {
    let s: TaskState = "research".parse().unwrap();
    assert_eq!(s, TaskState::Research);
    let s: TaskState = "ReSeArCh".parse().unwrap();
    assert_eq!(s, TaskState::Research);
}

#[test]
fn test_from_str_invalid() {
    let result: Result<TaskState, _> = "INVALID_STATE_XYZ".parse();
    assert!(result.is_err());
}

// ── Valid Transitions ──────────────────────────────────────────

#[test]
fn test_done_is_terminal() {
    assert!(TaskState::Done.valid_transitions().is_empty());
    assert!(!TaskState::Done.can_transition_to(TaskState::Queued));
}

#[test]
fn test_failed_can_retry_to_queued() {
    assert!(TaskState::Failed.can_transition_to(TaskState::Queued));
    assert!(!TaskState::Failed.can_transition_to(TaskState::Done));
    let transitions = TaskState::Failed.valid_transitions();
    assert_eq!(transitions, vec![TaskState::Queued]);
}

#[test]
fn test_backlog_to_queued() {
    assert!(TaskState::Backlog.can_transition_to(TaskState::Queued));
    assert!(TaskState::Backlog.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Backlog.can_transition_to(TaskState::Research));
}

#[test]
fn test_queued_has_many_transitions() {
    let q = TaskState::Queued;
    assert!(q.can_transition_to(TaskState::Research));
    assert!(q.can_transition_to(TaskState::Plan));
    assert!(q.can_transition_to(TaskState::Implement));
    assert!(q.can_transition_to(TaskState::Execute));
    assert!(q.can_transition_to(TaskState::Failed));
    // But cannot go back to Backlog
    assert!(!q.can_transition_to(TaskState::Backlog));
}

#[test]
fn test_pipeline_research_plan_implement() {
    assert!(TaskState::Research.can_transition_to(TaskState::Plan));
    assert!(TaskState::Research.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Research.can_transition_to(TaskState::Implement));

    assert!(TaskState::Plan.can_transition_to(TaskState::Implement));
    assert!(TaskState::Plan.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Plan.can_transition_to(TaskState::Research));

    assert!(TaskState::Implement.can_transition_to(TaskState::Validate));
    assert!(TaskState::Implement.can_transition_to(TaskState::Review));
    assert!(TaskState::Implement.can_transition_to(TaskState::Failed));
}

#[test]
fn test_validate_loop() {
    // Validate can go to Done or back to Implement
    assert!(TaskState::Validate.can_transition_to(TaskState::Done));
    assert!(TaskState::Validate.can_transition_to(TaskState::Implement));
    assert!(TaskState::Validate.can_transition_to(TaskState::Failed));
}

#[test]
fn test_legacy_review_to_implement() {
    assert!(TaskState::Review.can_transition_to(TaskState::Implement));
    assert!(TaskState::Review.can_transition_to(TaskState::Docs));
    assert!(TaskState::Review.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Review.can_transition_to(TaskState::Done));
}

#[test]
fn test_legacy_docs_to_learn() {
    assert!(TaskState::Docs.can_transition_to(TaskState::Learn));
    assert!(TaskState::Docs.can_transition_to(TaskState::Done));
    assert!(TaskState::Docs.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Docs.can_transition_to(TaskState::Implement));
}

#[test]
fn test_learn_terminal_like_done() {
    assert!(TaskState::Learn.can_transition_to(TaskState::Done));
    assert!(TaskState::Learn.can_transition_to(TaskState::Failed));
    assert!(!TaskState::Learn.can_transition_to(TaskState::Research));
    assert!(!TaskState::Learn.can_transition_to(TaskState::Implement));
}

// ── Specialty workflow transitions ─────────────────────────────

#[test]
fn test_migration_workflow() {
    assert!(TaskState::AddNew.can_transition_to(TaskState::Migrate));
    assert!(TaskState::Migrate.can_transition_to(TaskState::RemoveOld));
    assert!(TaskState::RemoveOld.can_transition_to(TaskState::Validate));
    assert!(TaskState::AddNew.can_transition_to(TaskState::Failed));
}

#[test]
fn test_bug_fix_workflow() {
    assert!(TaskState::Reproduce.can_transition_to(TaskState::Investigate));
    assert!(TaskState::Investigate.can_transition_to(TaskState::Implement));
}

#[test]
fn test_harden_workflow() {
    assert!(TaskState::Fix.can_transition_to(TaskState::Harden));
    assert!(TaskState::Harden.can_transition_to(TaskState::Validate));
    assert!(TaskState::Harden.can_transition_to(TaskState::Done));
    assert!(TaskState::Harden.can_transition_to(TaskState::Failed));
}

#[test]
fn test_test_workflow() {
    assert!(TaskState::Prepare.can_transition_to(TaskState::Test));
    assert!(TaskState::Test.can_transition_to(TaskState::Execute));
    assert!(TaskState::Execute.can_transition_to(TaskState::Cleanup));
    assert!(TaskState::Cleanup.can_transition_to(TaskState::Validate));
    assert!(TaskState::Cleanup.can_transition_to(TaskState::Done));
}

#[test]
fn test_spawn_workflow() {
    assert!(TaskState::Preparing.can_transition_to(TaskState::Spawning));
    assert!(TaskState::Spawning.can_transition_to(TaskState::Research));
    assert!(TaskState::Spawning.can_transition_to(TaskState::Implement));
    assert!(TaskState::Spawning.can_transition_to(TaskState::Failed));
}

// ── can_transition_to comprehensive ────────────────────────────

#[test]
fn test_all_states_can_transition_to_failed_except_done() {
    for state in TaskState::all() {
        if matches!(state, TaskState::Done) {
            assert!(!state.can_transition_to(TaskState::Failed));
        } else {
            assert!(state.can_transition_to(TaskState::Failed),
                "{:?} should be able to transition to Failed", state);
        }
    }
}

#[test]
fn test_all_states_have_transitions_or_are_terminal() {
    // Done is terminal; everyone else must have at least one transition
    for state in TaskState::all() {
        if matches!(state, TaskState::Done) {
            assert!(state.valid_transitions().is_empty());
        } else {
            assert!(!state.valid_transitions().is_empty(),
                "non-terminal state {:?} has no transitions", state);
        }
    }
}

// ── Serialization roundtrip ────────────────────────────────────

#[test]
fn test_task_state_serde_roundtrip() {
    for state in TaskState::all() {
        let json = serde_json::to_string(state).unwrap();
        let parsed: TaskState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, *state);
    }
}
