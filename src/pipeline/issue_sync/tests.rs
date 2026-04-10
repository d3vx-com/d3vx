//! Issue Sync Tests

use crate::pipeline::issue_sync::{
    ExternalIssue, IssueState, IssueTracker, SyncError, SyncResult, TrackerKind,
};

#[test]
fn test_external_issue_fields() {
    let issue = ExternalIssue {
        id: "42".into(),
        number: Some(42),
        title: "Bug".into(),
        body: Some("Details".into()),
        state: IssueState::Open,
        labels: vec!["bug".into()],
        assignee: Some("alice".into()),
        url: Some("https://github.com/o/r/issues/42".into()),
        tracker: TrackerKind::Github,
    };
    assert_eq!(issue.id, "42");
    assert_eq!(issue.number, Some(42));
    assert!(issue.labels.contains(&"bug".to_string()));
}

#[test]
fn test_issue_state_variants() {
    assert_eq!(IssueState::Open, IssueState::Open);
    assert_ne!(IssueState::Closed, IssueState::Cancelled);
}

#[test]
fn test_tracker_kind_variants() {
    assert_eq!(TrackerKind::Github, TrackerKind::Github);
    assert_ne!(TrackerKind::Github, TrackerKind::Linear);
}

#[test]
fn test_sync_result_default() {
    let result = SyncResult::default();
    assert_eq!(result.issues_fetched, 0);
    assert_eq!(result.tasks_created, 0);
    assert_eq!(result.tasks_updated, 0);
    assert!(result.errors.is_empty());
}

#[test]
fn test_sync_result_new() {
    let result = SyncResult::new();
    assert_eq!(result.issues_fetched, 0);
}

#[test]
fn test_sync_result_with_errors() {
    let result = SyncResult::with_errors(vec!["err1".into()]);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.issues_fetched, 0);
}

#[test]
fn test_sync_error_display() {
    let err = SyncError::Unavailable("down".into());
    assert!(err.to_string().contains("down"));

    let err = SyncError::ApiError("rate limit".into());
    assert!(err.to_string().contains("rate limit"));

    let err = SyncError::ParseError("bad json".into());
    assert!(err.to_string().contains("bad json"));

    let err = SyncError::NotConfigured;
    assert!(err.to_string().contains("Not configured"));
}

#[test]
fn test_github_tracker_constructor() {
    let tracker = IssueTracker::github("owner/repo".into());
    assert_eq!(tracker.kind, TrackerKind::Github);
    assert_eq!(tracker.repo.as_deref(), Some("owner/repo"));
    assert!(tracker.linear_api_key.is_none());
}

#[test]
fn test_linear_tracker_constructor() {
    let tracker = IssueTracker::linear("lin_api_key".into());
    assert_eq!(tracker.kind, TrackerKind::Linear);
    assert!(tracker.repo.is_none());
    assert_eq!(tracker.linear_api_key.as_deref(), Some("lin_api_key"));
}

#[test]
fn test_serialization_roundtrip() {
    let issue = ExternalIssue {
        id: "99".into(),
        number: Some(99),
        title: "Feature".into(),
        body: None,
        state: IssueState::Open,
        labels: vec!["enhancement".into()],
        assignee: None,
        url: None,
        tracker: TrackerKind::Github,
    };

    let json = serde_json::to_string(&issue).unwrap();
    let back: ExternalIssue = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "99");
    assert_eq!(back.state, IssueState::Open);
    assert_eq!(back.tracker, TrackerKind::Github);
}

#[test]
fn test_issue_state_serialization() {
    let json = serde_json::to_string(&IssueState::InProgress).unwrap();
    assert!(json.contains("in_progress"));
    let back: IssueState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, IssueState::InProgress);
}

#[test]
fn test_tracker_kind_serialization() {
    let json = serde_json::to_string(&TrackerKind::Github).unwrap();
    assert!(json.contains("github"));
    let back: TrackerKind = serde_json::from_str(&json).unwrap();
    assert_eq!(back, TrackerKind::Github);
}
