//! Tests for task queries, events, and type defaults

use super::enums::ExecutionMode;
use super::state_machine::TaskState;
use super::types::{NewTask, TaskListOptions, TaskUpdate};
use super::TaskStore;
use crate::store::database::Database;
use crate::store::workspace::ScopeMode;

fn create_test_db() -> Database {
    Database::in_memory().expect("Failed to create in-memory database")
}

/// Helper to create a basic NewTask with all required fields
fn new_task_builder() -> NewTask {
    NewTask {
        id: None,
        title: String::new(),
        description: None,
        state: None,
        priority: None,
        batch_id: None,
        max_retries: None,
        depends_on: None,
        metadata: None,
        project_path: None,
        agent_role: None,
        execution_mode: None,
        repo_root: None,
        task_scope_path: None,
        scope_mode: None,
        parent_task_id: None,
    }
}

#[test]
fn test_list_tasks() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    // Create tasks with different priorities
    for i in 0..5 {
        store
            .create(NewTask {
                id: Some(format!("list-task-{}", i)),
                title: format!("Task {}", i),
                state: Some(if i < 3 {
                    TaskState::Queued
                } else {
                    TaskState::Backlog
                }),
                priority: Some(i),
                ..new_task_builder()
            })
            .expect("Failed to create task");
    }

    // List all
    let all = store
        .list(TaskListOptions::default())
        .expect("Failed to list tasks");
    assert_eq!(all.len(), 5);

    // List by state
    let queued = store
        .list(TaskListOptions {
            state: Some(vec![TaskState::Queued]),
            ..Default::default()
        })
        .expect("Failed to list queued tasks");
    assert_eq!(queued.len(), 3);
}

#[test]
fn test_get_next_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("low-priority".to_string()),
            title: "Low Priority".to_string(),
            state: Some(TaskState::Queued),
            priority: Some(1),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    store
        .create(NewTask {
            id: Some("high-priority".to_string()),
            title: "High Priority".to_string(),
            state: Some(TaskState::Queued),
            priority: Some(10),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    let next = store
        .get_next()
        .expect("Failed to get next task")
        .expect("No next task");
    assert_eq!(next.id, "high-priority");
}

#[test]
fn test_retry_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("retry-test".to_string()),
            title: "Retry Test".to_string(),
            state: Some(TaskState::Failed),
            max_retries: Some(3),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    let retried = store.retry("retry-test").expect("Failed to retry task");
    assert!(retried);

    let task = store
        .get("retry-test")
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(task.state, TaskState::Queued);
    assert_eq!(task.retry_count, 1);
}

#[test]
fn test_task_counts() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("count-1".to_string()),
            title: "Count 1".to_string(),
            state: Some(TaskState::Queued),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    store
        .create(NewTask {
            id: Some("count-2".to_string()),
            title: "Count 2".to_string(),
            state: Some(TaskState::Done),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    let counts = store.get_counts().expect("Failed to get counts");
    assert_eq!(*counts.get(&TaskState::Queued).unwrap_or(&0), 1);
    assert_eq!(*counts.get(&TaskState::Done).unwrap_or(&0), 1);
}

#[test]
fn test_dependencies() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("dep-task".to_string()),
            title: "Dependency".to_string(),
            state: Some(TaskState::Backlog),
            ..new_task_builder()
        })
        .expect("Failed to create dependency task");

    store
        .create(NewTask {
            id: Some("dependent-task".to_string()),
            title: "Dependent".to_string(),
            depends_on: Some(vec!["dep-task".to_string()]),
            ..new_task_builder()
        })
        .expect("Failed to create dependent task");

    let met = store
        .are_dependencies_met("dependent-task")
        .expect("Failed to check dependencies");
    assert!(!met);

    store
        .transition("dep-task", TaskState::Queued)
        .expect("Failed to transition");
    store
        .transition("dep-task", TaskState::Done)
        .expect_err("Should fail - invalid transition");

    store
        .transition("dep-task", TaskState::Failed)
        .expect("Failed to transition to Failed");
    store
        .update(
            "dep-task",
            TaskUpdate {
                state: Some(TaskState::Done),
                ..Default::default()
            },
        )
        .expect("Failed to update");

    let met = store
        .are_dependencies_met("dependent-task")
        .expect("Failed to check dependencies");
    assert!(met);
}

#[test]
fn test_task_logs() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("log-test".to_string()),
            title: "Log Test".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    store
        .log_event(
            "log-test",
            "research",
            "file_read",
            Some(&serde_json::json!({ "path": "/test.txt" })),
            Some(150),
        )
        .expect("Failed to log event");

    let logs = store
        .get_logs("log-test", None)
        .expect("Failed to get logs");
    assert!(logs.len() >= 2); // created + file_read

    let research_logs = store
        .get_logs("log-test", Some("research"))
        .expect("Failed to get logs");
    assert_eq!(research_logs.len(), 1);
}

#[test]
fn test_execution_mode_defaults() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    let task = store
        .create(NewTask {
            id: Some("exec-mode-test".to_string()),
            title: "Execution Mode Test".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    assert_eq!(task.execution_mode, ExecutionMode::Auto);
    assert_eq!(task.scope_mode, ScopeMode::Repo);
}
