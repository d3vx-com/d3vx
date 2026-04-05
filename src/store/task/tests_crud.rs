//! Tests for task CRUD operations

use super::enums::ExecutionMode;
use super::state_machine::TaskState;
use super::types::{NewTask, TaskUpdate};
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
fn test_create_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    let task = store
        .create(NewTask {
            id: Some("task-001".to_string()),
            title: "Test Task".to_string(),
            description: Some("A test task".to_string()),
            state: Some(TaskState::Backlog),
            priority: Some(5),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    assert_eq!(task.id, "task-001");
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.state, TaskState::Backlog);
    assert_eq!(task.priority, 5);

    // Check that creation log was added
    let logs = store
        .get_logs("task-001", None)
        .expect("Failed to get logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event, "created");
}

#[test]
fn test_get_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("get-test".to_string()),
            title: "Get Test".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    let task = store
        .get("get-test")
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(task.title, "Get Test");
}

#[test]
fn test_transition_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("transition-test".to_string()),
            title: "Transition Test".to_string(),
            state: Some(TaskState::Backlog),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    // Valid transition: Backlog -> Queued
    store
        .transition("transition-test", TaskState::Queued)
        .expect("Failed to transition to Queued");

    let task = store
        .get("transition-test")
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(task.state, TaskState::Queued);

    // Invalid transition: Queued -> Backlog
    let result = store.transition("transition-test", TaskState::Backlog);
    assert!(result.is_err());
}

#[test]
fn test_update_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("update-test".to_string()),
            title: "Original Title".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    store
        .update(
            "update-test",
            TaskUpdate {
                title: Some("Updated Title".to_string()),
                priority: Some(10),
                ..Default::default()
            },
        )
        .expect("Failed to update task");

    let task = store
        .get("update-test")
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(task.title, "Updated Title");
    assert_eq!(task.priority, 10);
}

#[test]
fn test_delete_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("delete-test".to_string()),
            title: "Delete Me".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    store.delete("delete-test").expect("Failed to delete task");

    let result = store.get("delete-test").expect("Query should not error");
    assert!(result.is_none());
}

#[test]
fn test_state_transitions() {
    // Test valid transitions
    assert!(TaskState::Backlog.can_transition_to(TaskState::Queued));
    assert!(TaskState::Queued.can_transition_to(TaskState::Research));
    assert!(TaskState::Research.can_transition_to(TaskState::Plan));
    assert!(TaskState::Plan.can_transition_to(TaskState::Implement));
    assert!(TaskState::Implement.can_transition_to(TaskState::Validate));
    assert!(TaskState::Validate.can_transition_to(TaskState::Done));

    // Test invalid transitions
    assert!(!TaskState::Backlog.can_transition_to(TaskState::Done));
    assert!(!TaskState::Done.can_transition_to(TaskState::Queued));
    assert!(!TaskState::Queued.can_transition_to(TaskState::Backlog));
}

#[test]
fn test_execution_mode_explicit() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    let task = store
        .create(NewTask {
            id: Some("exec-mode-explicit".to_string()),
            title: "Execution Mode Explicit".to_string(),
            execution_mode: Some(ExecutionMode::Vex),
            scope_mode: Some(ScopeMode::Subdir),
            repo_root: Some("/path/to/repo".to_string()),
            task_scope_path: Some("/path/to/repo/src".to_string()),
            ..new_task_builder()
        })
        .expect("Failed to create task");

    assert_eq!(task.execution_mode, ExecutionMode::Vex);
    assert_eq!(task.scope_mode, ScopeMode::Subdir);
    assert_eq!(task.repo_root, Some("/path/to/repo".to_string()));
    assert_eq!(task.task_scope_path, Some("/path/to/repo/src".to_string()));
}

#[test]
fn test_parent_task() {
    let db = create_test_db();
    let store = TaskStore::new(&db);

    store
        .create(NewTask {
            id: Some("parent-task".to_string()),
            title: "Parent Task".to_string(),
            ..new_task_builder()
        })
        .expect("Failed to create parent task");

    let child = store
        .create(NewTask {
            id: Some("child-task".to_string()),
            title: "Child Task".to_string(),
            parent_task_id: Some("parent-task".to_string()),
            ..new_task_builder()
        })
        .expect("Failed to create child task");

    assert_eq!(child.parent_task_id, Some("parent-task".to_string()));
}
