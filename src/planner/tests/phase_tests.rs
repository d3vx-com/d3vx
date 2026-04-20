//! Tests for `PhaseSelection` construction, dedup, and name mapping.

use crate::pipeline::phases::Phase;
use crate::planner::phase::{phase_from_name, phase_name, PhaseSelection};
use crate::planner::PlannerError;

#[test]
fn empty_selection_is_empty() {
    let s = PhaseSelection::empty();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert!(s.phases().is_empty());
}

#[test]
fn from_phases_preserves_order() {
    let s = PhaseSelection::from_phases(vec![
        Phase::Plan,
        Phase::Implement,
        Phase::Docs,
    ]);
    assert_eq!(s.phases(), &[Phase::Plan, Phase::Implement, Phase::Docs]);
}

#[test]
fn from_phases_deduplicates_preserving_first_occurrence() {
    let s = PhaseSelection::from_phases(vec![
        Phase::Plan,
        Phase::Implement,
        Phase::Plan,      // dup: dropped
        Phase::Implement, // dup: dropped
        Phase::Docs,
    ]);
    assert_eq!(s.phases(), &[Phase::Plan, Phase::Implement, Phase::Docs]);
}

#[test]
fn try_from_phases_rejects_duplicates() {
    let result = PhaseSelection::try_from_phases(vec![Phase::Plan, Phase::Plan]);
    assert!(matches!(result, Err(PlannerError::DecisionInvalid(_))));
}

#[test]
fn contains_and_position_match_insertion_order() {
    let s = PhaseSelection::from_phases(vec![Phase::Research, Phase::Implement]);
    assert!(s.contains(Phase::Research));
    assert!(s.contains(Phase::Implement));
    assert!(!s.contains(Phase::Docs));
    assert_eq!(s.position(Phase::Research), Some(0));
    assert_eq!(s.position(Phase::Implement), Some(1));
    assert_eq!(s.position(Phase::Docs), None);
}

#[test]
fn phase_name_round_trips_all_variants() {
    for &p in Phase::all() {
        let name = phase_name(p);
        let back = phase_from_name(name).expect("valid name");
        assert_eq!(back, p, "round-trip failed for {name}");
    }
}

#[test]
fn phase_from_name_is_case_insensitive() {
    assert_eq!(phase_from_name("RESEARCH").unwrap(), Phase::Research);
    assert_eq!(phase_from_name("  Plan  ").unwrap(), Phase::Plan);
}

#[test]
fn phase_from_name_rejects_unknown() {
    assert!(matches!(
        phase_from_name("nonsense"),
        Err(PlannerError::UnknownPhase { .. })
    ));
}
