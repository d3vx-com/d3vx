//! Tests for the `Plan` data model and state transitions.

use crate::pipeline::phases::Phase;
use crate::planner::phase::PhaseSelection;
use crate::planner::plan::{Plan, PlanStatus, SectionState, Subtask};

fn mk_plan_pi() -> Plan {
    let sel = PhaseSelection::from_phases(vec![Phase::Plan, Phase::Implement]);
    Plan::new("pi-1", "Plan then implement", "original request text", sel)
}

#[test]
fn new_plan_creates_section_per_selected_phase() {
    let p = mk_plan_pi();
    assert_eq!(p.sections.len(), 2);
    assert_eq!(p.sections[0].phase, Phase::Plan);
    assert_eq!(p.sections[1].phase, Phase::Implement);
    assert!(p
        .sections
        .iter()
        .all(|s| s.state == SectionState::NotStarted));
    assert_eq!(p.status, PlanStatus::Draft);
}

#[test]
fn empty_selection_yields_completed_plan() {
    let p = Plan::new("empty", "trivial", "hello?", PhaseSelection::empty());
    assert!(p.sections.is_empty());
    assert_eq!(p.status, PlanStatus::Completed);
}

#[test]
fn first_pending_returns_lowest_not_started_index() {
    let mut p = mk_plan_pi();
    assert_eq!(p.first_pending_section_index(), Some(0));
    p.record_outcome(0, SectionState::Completed, "plan body").unwrap();
    assert_eq!(p.first_pending_section_index(), Some(1));
    p.record_outcome(1, SectionState::Completed, "implement body").unwrap();
    assert_eq!(p.first_pending_section_index(), None);
}

#[test]
fn record_outcome_advances_status_through_in_progress_to_completed() {
    let mut p = mk_plan_pi();
    p.record_outcome(0, SectionState::Completed, "done").unwrap();
    assert_eq!(p.status, PlanStatus::InProgress);
    p.record_outcome(1, SectionState::Completed, "done").unwrap();
    assert_eq!(p.status, PlanStatus::Completed);
    assert!(p.is_complete());
    assert!(!p.any_failed());
}

#[test]
fn failure_flips_plan_status_to_failed() {
    let mut p = mk_plan_pi();
    p.record_outcome(0, SectionState::Failed, "handler broke").unwrap();
    assert_eq!(p.status, PlanStatus::Failed);
    assert!(p.any_failed());
    assert!(!p.is_complete());
}

#[test]
fn record_outcome_rejects_out_of_range_index() {
    let mut p = mk_plan_pi();
    let err = p
        .record_outcome(99, SectionState::Completed, "oops")
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("out of range"), "unexpected msg: {msg}");
}

#[test]
fn set_plan_subtasks_attaches_to_plan_phase_only() {
    let mut p = mk_plan_pi();
    let subs = vec![
        Subtask::pending("step 1"),
        Subtask::completed("step 2 (done)"),
    ];
    p.set_plan_subtasks(subs.clone());
    let plan_section = p.sections.iter().find(|s| s.phase == Phase::Plan).unwrap();
    assert_eq!(plan_section.subtasks, subs);
    let impl_section = p
        .sections
        .iter()
        .find(|s| s.phase == Phase::Implement)
        .unwrap();
    assert!(impl_section.subtasks.is_empty());
}

#[test]
fn section_state_helpers_match_variants() {
    assert!(SectionState::Completed.is_done());
    assert!(!SectionState::NotStarted.is_done());
    assert!(SectionState::Completed.is_terminal());
    assert!(SectionState::Failed.is_terminal());
    assert!(!SectionState::InProgress.is_terminal());
}
