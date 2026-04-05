//! Tests for the session lifecycle state machine.

use super::probe::probe_agent_status;
use super::tracker::TRANSITIONS;
use super::types::{SessionPhase, TransitionCause, TransitionError};
use super::SessionTracker;

fn tracker_working(id: &str) -> SessionTracker {
    let mut t = SessionTracker::new(id.into());
    t.transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .unwrap();
    t.transition_to(SessionPhase::Working, TransitionCause::SystemAction)
        .unwrap();
    t
}

#[test]
fn test_provisioning_to_initializing() {
    let mut t = SessionTracker::new("s-1".into());
    assert!(t
        .transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .is_ok());
    assert_eq!(*t.current_phase(), SessionPhase::Initializing);
}

#[test]
fn test_full_happy_path() {
    let mut t = tracker_working("s-happy");
    for (phase, cause) in [
        (SessionPhase::PrOpen, TransitionCause::AutoDetected),
        (SessionPhase::CiRunning, TransitionCause::PollResult),
        (SessionPhase::CiPassed, TransitionCause::ExternalEvent),
        (SessionPhase::ReviewPending, TransitionCause::AutoDetected),
        (SessionPhase::ApprovedForMerge, TransitionCause::UserAction),
        (SessionPhase::Merging, TransitionCause::SystemAction),
        (SessionPhase::Merged, TransitionCause::AutoDetected),
    ] {
        t.transition_to(phase, cause).unwrap();
    }
    assert_eq!(*t.current_phase(), SessionPhase::Merged);
    assert!(t.is_terminal());
}

#[test]
fn test_approval_flow() {
    let mut t = tracker_working("s-appr");
    t.transition_to(
        SessionPhase::AwaitingApproval,
        TransitionCause::SystemAction,
    )
    .unwrap();
    t.transition_to(SessionPhase::Approved, TransitionCause::UserAction)
        .unwrap();
    t.transition_to(SessionPhase::Working, TransitionCause::SystemAction)
        .unwrap();
    assert_eq!(*t.current_phase(), SessionPhase::Working);
}

#[test]
fn test_ci_failure_recovery() {
    let mut t = tracker_working("s-ci");
    t.transition_to(SessionPhase::PrOpen, TransitionCause::AutoDetected)
        .unwrap();
    t.transition_to(SessionPhase::CiRunning, TransitionCause::PollResult)
        .unwrap();
    t.transition_to(SessionPhase::CiFailed, TransitionCause::ExternalEvent)
        .unwrap();
    t.transition_to(SessionPhase::Working, TransitionCause::SystemAction)
        .unwrap();
    assert_eq!(*t.current_phase(), SessionPhase::Working);
}

#[test]
fn test_stuck_recovery() {
    let mut t = tracker_working("s-stuck");
    t.transition_to(SessionPhase::Stuck, TransitionCause::TimeoutExpired)
        .unwrap();
    t.transition_to(SessionPhase::Working, TransitionCause::UserAction)
        .unwrap();
    assert_eq!(*t.current_phase(), SessionPhase::Working);
}

#[test]
fn test_crash_restart() {
    let mut t = SessionTracker::new("s-crash".into());
    t.transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .unwrap();
    t.transition_to(SessionPhase::Crashed, TransitionCause::SystemAction)
        .unwrap();
    t.transition_to(SessionPhase::Provisioning, TransitionCause::SystemAction)
        .unwrap();
    assert_eq!(*t.current_phase(), SessionPhase::Provisioning);
}

#[test]
fn test_invalid_direct_merge_from_working() {
    let mut t = tracker_working("s-bad");
    assert!(matches!(
        t.transition_to(SessionPhase::Merged, TransitionCause::AutoDetected),
        Err(TransitionError::InvalidTransition {
            from: SessionPhase::Working,
            to: SessionPhase::Merged
        })
    ));
}

#[test]
fn test_invalid_skip_ci_running() {
    let mut t = tracker_working("s-skip");
    t.transition_to(SessionPhase::PrOpen, TransitionCause::AutoDetected)
        .unwrap();
    assert!(matches!(
        t.transition_to(SessionPhase::CiPassed, TransitionCause::PollResult),
        Err(TransitionError::InvalidTransition {
            from: SessionPhase::PrOpen,
            to: SessionPhase::CiPassed
        })
    ));
}

#[test]
fn test_done_is_terminal() {
    let mut t = tracker_working("s-done");
    t.transition_to(SessionPhase::Done, TransitionCause::AutoDetected)
        .unwrap();
    assert!(t.is_terminal());
    assert!(matches!(
        t.transition_to(SessionPhase::Working, TransitionCause::UserAction),
        Err(TransitionError::AlreadyTerminal(SessionPhase::Done))
    ));
}

#[test]
fn test_cancelled_is_terminal() {
    let mut t = SessionTracker::new("s-cancel".into());
    t.transition_to(SessionPhase::Cancelled, TransitionCause::UserAction)
        .unwrap();
    assert!(t.is_terminal());
    assert!(matches!(
        t.transition_to(SessionPhase::Working, TransitionCause::UserAction),
        Err(TransitionError::AlreadyTerminal(SessionPhase::Cancelled))
    ));
}

#[test]
fn test_merged_is_terminal() {
    let mut t = tracker_working("s-mgd");
    t.transition_to(SessionPhase::PrOpen, TransitionCause::AutoDetected)
        .unwrap();
    t.transition_to(SessionPhase::Merged, TransitionCause::SystemAction)
        .unwrap();
    assert!(t.is_terminal());
}

#[test]
fn test_time_in_phase_advances() {
    let mut t = SessionTracker::new("s-time".into());
    assert!(t.time_in_phase().as_millis() < 500);
    t.transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .unwrap();
    assert!(t.time_in_phase().as_millis() < 500);
}

#[test]
fn test_history_records_transitions() {
    let mut t = SessionTracker::new("s-hist".into());
    assert!(t.history().is_empty());
    t.transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .unwrap();
    assert_eq!(t.history().len(), 1);
    assert_eq!(t.history()[0].from, SessionPhase::Provisioning);
    t.transition_to(SessionPhase::Working, TransitionCause::SystemAction)
        .unwrap();
    assert_eq!(t.history().len(), 2);
    assert_eq!(t.history()[1].from, SessionPhase::Initializing);
}

#[test]
fn test_metadata_transition_count() {
    let mut t = SessionTracker::new("s-cnt".into());
    assert_eq!(t.metadata().transition_count, 0);
    t.transition_to(SessionPhase::Initializing, TransitionCause::AutoDetected)
        .unwrap();
    assert_eq!(t.metadata().transition_count, 1);
    t.transition_to(SessionPhase::Working, TransitionCause::SystemAction)
        .unwrap();
    assert_eq!(t.metadata().transition_count, 2);
}

#[test]
fn test_summarize_basic() {
    let s = tracker_working("s-sum").summarize();
    assert_eq!(s.session_id, "s-sum");
    assert_eq!(s.phase, SessionPhase::Working);
    assert!(s.duration_secs >= 0.0);
    assert!(s.pr_url.is_none());
    assert_eq!(s.cost_usd, 0.0);
}

#[test]
fn test_summarize_with_custom_data() {
    let mut t = SessionTracker::new("s-sc".into());
    t.metadata_mut()
        .custom_data
        .insert("pr_url".into(), "https://g.co/pull/1".into());
    t.metadata_mut()
        .custom_data
        .insert("branch".into(), "feat/x".into());
    t.metadata_mut()
        .custom_data
        .insert("cost_usd".into(), "0.42".into());
    let s = t.summarize();
    assert_eq!(s.pr_url, Some("https://g.co/pull/1".into()));
    assert_eq!(s.branch, Some("feat/x".into()));
    assert!((s.cost_usd - 0.42).abs() < f64::EPSILON);
}

#[test]
fn test_actionable_phases() {
    for p in [
        SessionPhase::AwaitingApproval,
        SessionPhase::NeedsInput,
        SessionPhase::ReviewPending,
        SessionPhase::ChangesRequested,
        SessionPhase::Stuck,
        SessionPhase::CiFailed,
    ] {
        assert!(p.is_actionable(), "{p:?} should be actionable");
    }
    for p in [
        SessionPhase::Working,
        SessionPhase::Merged,
        SessionPhase::Provisioning,
    ] {
        assert!(!p.is_actionable(), "{p:?} should not be actionable");
    }
}

#[test]
fn test_all_defined_transitions_are_valid() {
    for (from, targets) in TRANSITIONS.iter() {
        for to in targets {
            let mut t = SessionTracker::with_phase("s-g".into(), *from);
            assert!(
                t.transition_to(*to, TransitionCause::SystemAction).is_ok(),
                "Expected {from:?} -> {to:?} to be valid"
            );
        }
    }
}

#[test]
fn test_terminal_phases_have_no_targets() {
    for p in [
        SessionPhase::Done,
        SessionPhase::Merged,
        SessionPhase::Cancelled,
    ] {
        assert!(
            TRANSITIONS
                .get(&p)
                .map_or(true, |v: &Vec<SessionPhase>| v.is_empty()),
            "{p:?} should have no targets"
        );
    }
}

#[test]
fn test_probe_agent_status_mapping() {
    assert_eq!(probe_agent_status(true, None), SessionPhase::Working);
    assert_eq!(probe_agent_status(false, Some(0)), SessionPhase::Done);
    assert_eq!(probe_agent_status(false, Some(1)), SessionPhase::Crashed);
    assert_eq!(probe_agent_status(false, None), SessionPhase::Orphaned);
}

#[test]
fn test_with_phase_constructor() {
    let t = SessionTracker::with_phase("s-rec".into(), SessionPhase::CiRunning);
    assert_eq!(*t.current_phase(), SessionPhase::CiRunning);
    assert_eq!(t.session_id(), "s-rec");
    assert!(t.history().is_empty());
}

#[test]
fn test_self_transition_is_valid() {
    let mut t = SessionTracker::new("s-self".into());
    assert!(t
        .transition_to(SessionPhase::Provisioning, TransitionCause::SystemAction)
        .is_ok());
}

#[test]
fn test_session_phase_display() {
    assert_eq!(format!("{}", SessionPhase::Working), "working");
    assert_eq!(format!("{}", SessionPhase::CiRunning), "ci_running");
    assert_eq!(format!("{}", SessionPhase::PrOpen), "pr_open");
    assert_eq!(
        format!("{}", SessionPhase::ApprovedForMerge),
        "approved_for_merge"
    );
}
