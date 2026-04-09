//! Integration tests for GitHub webhook handling

use crate::pipeline::github::{CIStatus, GitHubEvent, GitHubIntegration};
use crate::pipeline::phases::Priority;

#[test]
fn test_issue_opened_to_task() {
    let mut integration = GitHubIntegration::with_defaults();
    integration.add_trigger_label("bug");

    let event = GitHubEvent::IssueOpened {
        number: 42,
        repository: "owner/repo".to_string(),
        author: "user".to_string(),
        title: "Fix the bug".to_string(),
        body: Some("Details here".to_string()),
        labels: vec!["bug".to_string()],
    };

    let task = integration.process_webhook(event).unwrap();
    assert!(task.is_some());

    let task = task.unwrap();
    assert!(task.title.contains("Fix the bug"));
}

#[test]
fn test_issue_without_trigger_label() {
    let integration = GitHubIntegration::with_defaults();

    let event = GitHubEvent::IssueOpened {
        number: 42,
        repository: "owner/repo".to_string(),
        author: "user".to_string(),
        title: "Fix the bug".to_string(),
        body: Some("Details here".to_string()),
        labels: vec!["question".to_string()],
    };

    let input = integration.event_to_intake(&event).unwrap();
    assert!(input.is_none());
}

#[test]
fn test_ci_failure_to_task() {
    let mut integration = GitHubIntegration::with_defaults();

    let event = GitHubEvent::CIStatusChanged {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        status: CIStatus::Failure,
        context: "ci/tests".to_string(),
        description: Some("Tests failed".to_string()),
        target_url: Some("https://example.com/build/1".to_string()),
    };

    let task = integration.process_webhook(event).unwrap();
    assert!(task.is_some());

    let task = task.unwrap();
    assert_eq!(task.priority, Priority::Critical);
}

#[test]
fn test_ci_success_no_task() {
    let integration = GitHubIntegration::with_defaults();

    let event = GitHubEvent::CIStatusChanged {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        status: CIStatus::Success,
        context: "ci/tests".to_string(),
        description: None,
        target_url: None,
    };

    let input = integration.event_to_intake(&event).unwrap();
    assert!(input.is_none());
}

#[test]
fn test_deduplication() {
    let mut integration = GitHubIntegration::with_defaults();
    integration.add_trigger_label("bug");

    let event = GitHubEvent::IssueOpened {
        number: 42,
        repository: "owner/repo".to_string(),
        author: "user".to_string(),
        title: "Fix the bug".to_string(),
        body: Some("Details".to_string()),
        labels: vec!["bug".to_string()],
    };

    let task1 = integration.process_webhook(event.clone()).unwrap();
    assert!(task1.is_some());

    let task2 = integration.process_webhook(event).unwrap();
    assert!(task2.is_none());
}
