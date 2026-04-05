//! Task intake layer tests

use super::super::phases::Priority;
use super::layer::TaskIntake;
use super::types::*;

#[test]
fn test_task_source_label() {
    let source = TaskSource::GitHubIssue {
        number: 42,
        repository: "owner/repo".to_string(),
        author: "user".to_string(),
    };
    assert_eq!(source.label(), "GitHub Issue #42 (owner/repo)");
}

#[test]
fn test_intake_from_chat() {
    let input = TaskIntakeInput::from_chat("Test task", "Do something");
    assert_eq!(input.source, TaskSource::Chat);
    assert_eq!(input.title, "Test task");
}

#[test]
fn test_intake_from_github_issue() {
    let input = TaskIntakeInput::from_github_issue(
        123,
        "owner/repo",
        "author",
        "Bug: Something broke",
        "Here's what happened...",
    );
    assert!(matches!(input.source, TaskSource::GitHubIssue { .. }));
    assert!(input.tags.contains(&"github".to_string()));
}

#[test]
fn test_normalize_to_task() {
    let intake = TaskIntake::new("TEST");
    let input = TaskIntakeInput::from_chat("Test", "Instruction").with_priority(Priority::High);

    let task = intake.normalize_to_task(input).unwrap();
    assert!(task.id.starts_with("TEST-"));
    assert_eq!(task.title, "Test");
    assert_eq!(task.priority, Priority::High);
}

#[test]
fn test_priority_inference_critical() {
    let intake = TaskIntake::new("TEST");
    let input = TaskIntakeInput::from_chat(
        "URGENT: Production down",
        "The system is broken and needs immediate fix",
    );

    let task = intake.normalize_to_task(input).unwrap();
    assert_eq!(task.priority, Priority::Critical);
}

#[test]
fn test_ci_failure_priority() {
    let intake = TaskIntake::new("TEST");
    let input = TaskIntakeInput::from_ci_failure("pipeline-123", "main", "abc123", "Tests failed");

    let task = intake.normalize_to_task(input).unwrap();
    assert_eq!(task.priority, Priority::Critical);
}

#[test]
fn test_validate_intake_empty_instruction() {
    let intake = TaskIntake::new("TEST");
    let input = TaskIntakeInput::from_chat("Test", "");

    let result = intake.validate_intake(&input);
    assert!(result.is_err());
}

#[test]
fn test_validate_intake_warnings() {
    let intake = TaskIntake::new("TEST");
    let input = TaskIntakeInput::from_chat("Test", "x".repeat(15000))
        .with_dependencies(vec!["OTHER-001".to_string()]);

    let warnings = intake.validate_intake(&input).unwrap();
    assert!(warnings.iter().any(|w| w.contains("very long")));
    assert!(warnings.iter().any(|w| w.contains("dependencies")));
}
