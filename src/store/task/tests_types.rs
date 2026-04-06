//! Tests for store task enum types

use super::*;

#[test]
fn test_task_state_variants() {
    let states = vec![
        TaskState::Pending,
        TaskState::InProgress,
        TaskState::Blocked,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
    ];
    for state in states {
        let _ = state.to_string();
        let _ = serde_yaml::to_string(&state).unwrap();
    }
}

#[test]
fn test_task_state_display() {
    assert_eq!(TaskState::Pending.to_string(), "pending");
    assert_eq!(TaskState::Completed.to_string(), "completed");
}

#[test]
fn test_task_priority_variants() {
    let priorities = vec![
        TaskPriority::Low,
        TaskPriority::Medium,
        TaskPriority::High,
        TaskPriority::Critical,
    ];
    for priority in priorities {
        let _ = priority.to_string();
        let json = serde_json::to_string(&priority).unwrap();
        let parsed: TaskPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, parsed);
    }
}

#[test]
fn test_task_from_str() {
    assert!(TaskState::try_from("pending").is_ok());
    assert!(TaskState::try_from("completed").is_ok());
    assert!(TaskState::try_from("invalid_state_xyz").is_err());
}
