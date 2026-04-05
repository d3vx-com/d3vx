//! Tests for PR Lifecycle Automation

use super::manager::PrLifecycleManager;
use super::types::*;

fn check(name: &str, status: CheckConclusion) -> CiStatus {
    CiStatus {
        check_name: name.into(),
        status,
        url: None,
    }
}

fn review(name: &str, state: ReviewState) -> ReviewInfo {
    ReviewInfo {
        reviewer: name.into(),
        state,
        body: None,
    }
}

#[test]
fn test_default_and_new() {
    let d = PrMetadata::default();
    assert_eq!(d.state, PrState::NotCreated);
    assert!(d.pr_number.is_none() && d.ci_checks.is_empty() && d.reviews.is_empty());
    let n = PrMetadata::new("feat/x");
    assert_eq!(n.branch, "feat/x");
    assert_eq!(n.state, PrState::NotCreated);
}

#[test]
fn test_ci_checks() {
    let mut m = PrMetadata::new("b");
    assert!(!m.ci_passed() && !m.ci_failed());
    m.ci_checks = vec![
        check("build", CheckConclusion::Success),
        check("test", CheckConclusion::Success),
    ];
    assert!(m.ci_passed() && !m.ci_failed());
    m.ci_checks.push(check("lint", CheckConclusion::Failure));
    assert!(m.ci_failed() && !m.ci_passed());
    m.ci_checks = vec![check("e2e", CheckConclusion::TimedOut)];
    assert!(m.ci_failed());
    m.ci_checks = vec![check("ci", CheckConclusion::Cancelled)];
    assert!(m.ci_failed());
}

#[test]
fn test_reviews() {
    let mut m = PrMetadata::new("b");
    m.reviews = vec![
        review("alice", ReviewState::Commented),
        review("bob", ReviewState::Approved),
    ];
    assert!(m.has_approved_review() && !m.has_changes_requested());
    assert_eq!(m.pending_review_comments().len(), 0);
    m.reviews = vec![
        review("carol", ReviewState::ChangesRequested),
        review("dave", ReviewState::Pending),
    ];
    assert!(!m.has_approved_review() && m.has_changes_requested());
    assert_eq!(m.pending_review_comments().len(), 1);
    assert_eq!(m.pending_review_comments()[0].reviewer, "dave");
}

#[test]
fn test_mergeable_requires_all_three() {
    let mut m = PrMetadata::new("b");
    assert!(!m.is_mergeable());
    m.mergeable = Some(true);
    m.ci_checks = vec![check("ci", CheckConclusion::Success)];
    assert!(!m.is_mergeable()); // no approval
    m.reviews = vec![review("alice", ReviewState::Approved)];
    assert!(m.is_mergeable());
    m.mergeable = Some(false);
    assert!(!m.is_mergeable()); // gh says not mergeable
}

#[test]
fn test_enums_and_errors() {
    assert_eq!(PrState::NotCreated, PrState::NotCreated);
    assert_ne!(PrState::Open, PrState::Merged);
    assert_eq!(CheckConclusion::Pending, CheckConclusion::Pending);
    assert_eq!(ReviewState::Approved, ReviewState::Approved);
    assert!(PrError::CliNotAvailable("x".into())
        .to_string()
        .contains("x"));
    assert!(PrError::NoRepo.to_string().contains("No repository"));
}

#[test]
fn test_manager_helpers() {
    assert!(PrLifecycleManager::new(Some("o/r".into())).repo.is_some());
    assert!(PrLifecycleManager::new(None).repo.is_none());
    let mgr = PrLifecycleManager::new(None);
    assert!(mgr.pr_ref(&PrMetadata::new("b")).is_err());
    let mut m = PrMetadata::new("b");
    m.pr_number = Some(42);
    assert_eq!(mgr.pr_ref(&m).unwrap(), "42");
}

#[test]
fn test_serialization_roundtrip() {
    let mut m = PrMetadata::new("feat/rt");
    m.pr_number = Some(7);
    m.state = PrState::CiPassed;
    let json = serde_json::to_string(&m).unwrap();
    let back: PrMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(
        (back.pr_number, back.state, back.branch),
        (Some(7), PrState::CiPassed, "feat/rt".to_string())
    );
}
