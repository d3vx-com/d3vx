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
    assert_eq!(task.phase, Phase::Ideation);

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

#[test]
fn test_set_phase_updates_timestamp() {
    let mut task = Task::new("T-1", "Test", "Instruction");
    let before = task.updated_at;
    std::thread::sleep(std::time::Duration::from_millis(10));
    task.set_phase(Phase::Plan);
    assert_eq!(task.phase, Phase::Plan);
    assert!(task.updated_at > before);
}

#[test]
fn test_set_status_updates_timestamp() {
    let mut task = Task::new("T-1", "Test", "Instruction");
    task.set_status(TaskStatus::Completed);
    assert_eq!(task.status, TaskStatus::Completed);
}

#[test]
fn test_task_builder_branch_and_project_root() {
    let task = Task::new("T-1", "Test", "Instruction")
        .with_branch("feat/auth")
        .with_project_root("/root/project");

    assert_eq!(task.branch, Some("feat/auth".to_string()));
    assert_eq!(task.project_root, Some("/root/project".to_string()));
}

#[test]
fn test_task_builder_chained() {
    let task = Task::new("T-1", "Test", "Instruction")
        .with_worktree("/wt")
        .with_branch("feat/x")
        .with_project_root("/root")
        .with_phase(Phase::Draft)
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::Critical);

    assert_eq!(task.worktree_path, Some("/wt".to_string()));
    assert_eq!(task.branch, Some("feat/x".to_string()));
    assert_eq!(task.phase, Phase::Draft);
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.priority, Priority::Critical);
}

#[test]
fn test_phase_label() {
    assert_eq!(Phase::Research.label(), "Research");
    assert_eq!(Phase::Ideation.label(), "Ideation");
    assert_eq!(Phase::Plan.label(), "Plan");
    assert_eq!(Phase::Draft.label(), "Draft");
    assert_eq!(Phase::Implement.label(), "Implement");
    assert_eq!(Phase::Review.label(), "Review");
    assert_eq!(Phase::Docs.label(), "Docs");
}

#[test]
fn test_phase_commit_prefix() {
    assert_eq!(Phase::Research.commit_prefix(), "chore(research)");
    assert_eq!(Phase::Plan.commit_prefix(), "docs(plan)");
    assert_eq!(Phase::Draft.commit_prefix(), "feat(draft)");
    assert_eq!(Phase::Implement.commit_prefix(), "feat");
    assert_eq!(Phase::Review.commit_prefix(), "chore(review)");
    assert_eq!(Phase::Docs.commit_prefix(), "docs");
}

#[test]
fn test_task_status_display() {
    assert_eq!(TaskStatus::Backlog.to_string(), "BACKLOG");
    assert_eq!(TaskStatus::Queued.to_string(), "QUEUED");
    assert_eq!(TaskStatus::InProgress.to_string(), "IN_PROGRESS");
    assert_eq!(TaskStatus::Completed.to_string(), "COMPLETED");
    assert_eq!(TaskStatus::Failed.to_string(), "FAILED");
    assert_eq!(TaskStatus::Cancelled.to_string(), "CANCELLED");
    assert_eq!(TaskStatus::Unknown.to_string(), "UNKNOWN");
}

#[test]
fn test_task_status_is_active() {
    assert!(TaskStatus::Queued.is_active());
    assert!(TaskStatus::InProgress.is_active());
    assert!(!TaskStatus::Backlog.is_active());
    assert!(!TaskStatus::Completed.is_active());
    assert!(!TaskStatus::Cancelled.is_active());
}

#[test]
fn test_task_status_serialization_roundtrip() {
    let statuses = [
        TaskStatus::Backlog,
        TaskStatus::Queued,
        TaskStatus::InProgress,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
        TaskStatus::Unknown,
    ];
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }
}

#[test]
fn test_phase_display() {
    assert_eq!(Phase::Research.to_string(), "RESEARCH");
    assert_eq!(Phase::Ideation.to_string(), "IDEATION");
    assert_eq!(Phase::Plan.to_string(), "PLAN");
    assert_eq!(Phase::Draft.to_string(), "DRAFT");
    assert_eq!(Phase::Implement.to_string(), "IMPLEMENT");
    assert_eq!(Phase::Review.to_string(), "REVIEW");
    assert_eq!(Phase::Docs.to_string(), "DOCS");
}

#[test]
fn test_task_serialization_excludes_null_fields() {
    let task = Task::new("T-1", "Test", "Instruction");
    let json = serde_json::to_value(&task).unwrap();
    // Skip-serialized optional fields are absent; metadata uses skip_serializing_if is_null
    assert!(
        json.as_object().unwrap().get("worktree_path").is_none()
            || json.as_object().unwrap()["worktree_path"].is_null()
    );
}

#[test]
fn test_phase_context_construction() {
    let task = Task::new("T-1", "Test", "Instruction");
    let ctx = PhaseContext::new(task.clone(), "/root", "/wt");

    assert_eq!(ctx.task.id, "T-1");
    assert_eq!(ctx.project_root, "/root");
    assert_eq!(ctx.worktree_path, "/wt");
    assert!(ctx.agent_rules.is_none());
    assert!(ctx.memory_context.is_none());
    assert!(ctx.session_id.is_none());
}

#[test]
fn test_phase_context_builder_chain() {
    let task = Task::new("T-1", "Test", "Instruction");
    let ctx = PhaseContext::new(task, "/root", "/wt")
        .with_agent_rules("rule-content")
        .with_memory_context("mem-ctx")
        .with_ignore_instruction("ignore-this")
        .with_session_id("session-abc");

    assert_eq!(ctx.agent_rules, Some("rule-content".to_string()));
    assert_eq!(ctx.memory_context, Some("mem-ctx".to_string()));
    assert_eq!(ctx.ignore_instruction, Some("ignore-this".to_string()));
    assert_eq!(ctx.session_id, Some("session-abc".to_string()));
}

#[test]
fn test_task_advance_phase_from_each_phase() {
    for phase in Phase::all() {
        if matches!(phase, Phase::Docs) {
            let mut task = Task::new("T-1", "T", "I").with_phase(*phase);
            assert!(!task.advance_phase());
            assert_eq!(task.phase, Phase::Docs);
        } else {
            let mut task = Task::new("T-1", "T", "I").with_phase(*phase);
            assert!(task.advance_phase());
            assert_ne!(task.phase, *phase);
        }
    }
}

#[test]
fn test_priority_display() {
    assert_eq!(Priority::Low.to_string(), "low");
    assert_eq!(Priority::Normal.to_string(), "normal");
    assert_eq!(Priority::High.to_string(), "high");
    assert_eq!(Priority::Critical.to_string(), "critical");
}

#[test]
fn test_task_retry_boundary() {
    let mut task = Task::new("T-1", "T", "I");
    assert_eq!(task.retry_count, 0);
    assert_eq!(task.max_retries, 3);

    for i in 1..=3 {
        assert!(task.can_retry());
        task.increment_retry();
        assert_eq!(task.retry_count, i);
    }
    assert!(!task.can_retry());
}
