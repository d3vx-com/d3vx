//! Pipeline phase type tests

use super::super::phases::task::{PhaseContext, Task};
use super::super::phases::types::*;

#[test]
fn test_phase_ordering() {
    let phases = Phase::all();
    assert_eq!(phases.len(), 7);
    assert_eq!(phases[0], Phase::Research);
    assert_eq!(phases[6], Phase::Docs);
}

#[test]
fn test_phase_next() {
    assert_eq!(Phase::Research.next(), Some(Phase::Ideation));
    assert_eq!(Phase::Ideation.next(), Some(Phase::Plan));
    assert_eq!(Phase::Plan.next(), Some(Phase::Draft));
    assert_eq!(Phase::Draft.next(), Some(Phase::Review));
    assert_eq!(Phase::Review.next(), Some(Phase::Implement));
    assert_eq!(Phase::Implement.next(), Some(Phase::Docs));
    assert_eq!(Phase::Docs.next(), None);
}

#[test]
fn test_phase_is_final() {
    assert!(!Phase::Research.is_final());
    assert!(!Phase::Implement.is_final());
    assert!(Phase::Docs.is_final());
}

#[test]
fn test_phase_from_str_ignore_case() {
    assert_eq!(
        Phase::from_str_ignore_case("research"),
        Some(Phase::Research)
    );
    assert_eq!(
        Phase::from_str_ignore_case("RESEARCH"),
        Some(Phase::Research)
    );
    assert_eq!(
        Phase::from_str_ignore_case("Implement"),
        Some(Phase::Implement)
    );
    assert_eq!(Phase::from_str_ignore_case("unknown"), None);
}

#[test]
fn test_task_status_is_terminal() {
    assert!(!TaskStatus::Backlog.is_terminal());
    assert!(!TaskStatus::Queued.is_terminal());
    assert!(!TaskStatus::InProgress.is_terminal());
    assert!(TaskStatus::Completed.is_terminal());
    assert!(TaskStatus::Failed.is_terminal());
}

#[test]
fn test_task_creation() {
    let task = Task::new("TASK-001", "Test task", "Test instruction");
    assert_eq!(task.id, "TASK-001");
    assert_eq!(task.title, "Test task");
    assert_eq!(task.instruction, "Test instruction");
    assert_eq!(task.phase, Phase::Research);
    assert_eq!(task.status, TaskStatus::Backlog);
    assert_eq!(task.priority, Priority::Normal);
}

#[test]
fn test_task_builder() {
    let task = Task::new("TASK-001", "Test", "Test instruction")
        .with_phase(Phase::Implement)
        .with_status(TaskStatus::InProgress)
        .with_priority(Priority::High)
        .with_worktree("/path/to/worktree");

    assert_eq!(task.phase, Phase::Implement);
    assert_eq!(task.status, TaskStatus::InProgress);
    assert_eq!(task.priority, Priority::High);
    assert_eq!(task.worktree_path, Some("/path/to/worktree".to_string()));
}

#[test]
fn test_task_advance_phase() {
    let mut task = Task::new("TASK-001", "Test", "Test instruction").with_phase(Phase::Research);

    assert!(task.advance_phase());
    assert_eq!(task.phase, Phase::Plan);

    assert!(task.advance_phase());
    assert_eq!(task.phase, Phase::Draft);

    assert!(task.advance_phase());
    assert_eq!(task.phase, Phase::Review);

    assert!(task.advance_phase());
    assert_eq!(task.phase, Phase::Implement);

    assert!(task.advance_phase());
    assert_eq!(task.phase, Phase::Docs);

    // Can't advance past Docs
    assert!(!task.advance_phase());
    assert_eq!(task.phase, Phase::Docs);
}

#[test]
fn test_task_retry() {
    let mut task = Task::new("TASK-001", "Test", "Test instruction");
    task.max_retries = 2;

    assert!(task.can_retry());
    task.increment_retry();
    assert_eq!(task.retry_count, 1);
    assert!(task.can_retry());

    task.increment_retry();
    assert_eq!(task.retry_count, 2);
    assert!(!task.can_retry());
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::Critical > Priority::High);
    assert!(Priority::High > Priority::Normal);
    assert!(Priority::Normal > Priority::Low);
}

#[test]
fn test_task_serialization() {
    let task = Task::new("TASK-001", "Test task", "Test instruction")
        .with_phase(Phase::Implement)
        .with_status(TaskStatus::InProgress);

    let json = serde_json::to_string(&task).unwrap();
    assert!(json.contains("TASK-001"));
    assert!(json.contains("IMPLEMENT"));
    assert!(json.contains("IN_PROGRESS"));

    let deserialized: Task = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, task.id);
    assert_eq!(deserialized.phase, task.phase);
    assert_eq!(deserialized.status, task.status);
}
