//! Task run store tests

use super::store::TaskRunStore;
use super::types::{NewTaskRun, RunStatus};
use crate::store::database::Database;
use crate::store::task::{NewTask, TaskStore};

fn create_test_db() -> Database {
    Database::in_memory().expect("Failed to create in-memory database")
}

/// Helper to create a parent task for FK constraint
fn create_parent_task(db: &Database, task_id: &str) {
    let task_store = TaskStore::new(db);
    task_store
        .create(NewTask {
            id: Some(task_id.to_string()),
            title: "Parent task".to_string(),
            description: Some("Parent task for test".to_string()),
            ..Default::default()
        })
        .expect("Failed to create parent task");
}

#[test]
fn test_create_task_run() {
    let db = create_test_db();
    create_parent_task(&db, "task-001");
    let store = TaskRunStore::new(&db);

    let run = store
        .create(NewTaskRun {
            id: Some("run-001".to_string()),
            task_id: "task-001".to_string(),
            attempt_number: 1,
            worker_id: None,
            workspace_id: None,
            metrics: None,
        })
        .expect("Failed to create run");

    assert_eq!(run.id, "run-001");
    assert_eq!(run.task_id, "task-001");
    assert_eq!(run.attempt_number, 1);
    assert_eq!(run.status, RunStatus::Pending);
}

#[test]
fn test_update_run_status() {
    let db = create_test_db();
    create_parent_task(&db, "task-002");
    let store = TaskRunStore::new(&db);

    store
        .create(NewTaskRun {
            id: Some("run-002".to_string()),
            task_id: "task-002".to_string(),
            attempt_number: 1,
            worker_id: None,
            workspace_id: None,
            metrics: None,
        })
        .expect("Failed to create run");

    store
        .update_status("run-002", RunStatus::Running)
        .expect("Failed to update status");

    let run = store
        .get("run-002")
        .expect("Failed to get run")
        .expect("Run not found");
    assert_eq!(run.status, RunStatus::Running);
    assert!(run.ended_at.is_none());

    // Complete the run
    store
        .update_status("run-002", RunStatus::Completed)
        .expect("Failed to update status");

    let run = store
        .get("run-002")
        .expect("Failed to get run")
        .expect("Run not found");
    assert_eq!(run.status, RunStatus::Completed);
    assert!(run.ended_at.is_some());
}

#[test]
fn test_list_runs_for_task() {
    let db = create_test_db();
    create_parent_task(&db, "task-003");
    let store = TaskRunStore::new(&db);

    // Create multiple runs for the same task
    for i in 1..=3 {
        store
            .create(NewTaskRun {
                id: Some(format!("run-{}", i)),
                task_id: "task-003".to_string(),
                attempt_number: i,
                worker_id: None,
                workspace_id: None,
                metrics: None,
            })
            .expect("Failed to create run");
    }

    let runs = store
        .list_runs_for_task("task-003")
        .expect("Failed to list runs");
    assert_eq!(runs.len(), 3);
    // Should be in descending order by attempt_number
    assert_eq!(runs[0].attempt_number, 3);
}

#[test]
fn test_get_next_attempt_number() {
    let db = create_test_db();
    create_parent_task(&db, "task-004");
    let store = TaskRunStore::new(&db);

    // No runs yet
    let next = store
        .get_next_attempt_number("task-004")
        .expect("Failed to get next attempt");
    assert_eq!(next, 1);

    // Create a run
    store
        .create(NewTaskRun {
            id: None,
            task_id: "task-004".to_string(),
            attempt_number: 1,
            worker_id: None,
            workspace_id: None,
            metrics: None,
        })
        .expect("Failed to create run");

    let next = store
        .get_next_attempt_number("task-004")
        .expect("Failed to get next attempt");
    assert_eq!(next, 2);
}
