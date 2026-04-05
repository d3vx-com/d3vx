//! Event store tests

use super::store::{emit_state_change, EventStore};
use super::types::{EventType, NewEvent};
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
fn test_append_event() {
    let db = create_test_db();
    create_parent_task(&db, "task-001");
    let store = EventStore::new(&db);

    let event = store
        .append(NewEvent {
            task_id: "task-001".to_string(),
            run_id: None,
            event_type: EventType::TaskCreated,
            data: Some(serde_json::json!({ "title": "Test task" })),
        })
        .expect("Failed to append event");

    assert!(event.id > 0);
    assert_eq!(event.task_id, "task-001");
    assert_eq!(event.event_type, EventType::TaskCreated);
}

#[test]
fn test_get_events_for_task() {
    let db = create_test_db();
    create_parent_task(&db, "task-002");
    let store = EventStore::new(&db);

    // Create multiple events
    store
        .append(NewEvent {
            task_id: "task-002".to_string(),
            run_id: None,
            event_type: EventType::TaskCreated,
            data: None,
        })
        .expect("Failed to append");

    store
        .append(NewEvent {
            task_id: "task-002".to_string(),
            run_id: None,
            event_type: EventType::TaskQueued,
            data: None,
        })
        .expect("Failed to append");

    let events = store
        .get_for_task("task-002")
        .expect("Failed to get events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, EventType::TaskCreated);
    assert_eq!(events[1].event_type, EventType::TaskQueued);
}

#[test]
fn test_state_change_helper() {
    let db = create_test_db();
    create_parent_task(&db, "task-003");
    let store = EventStore::new(&db);

    emit_state_change(&store, "task-003", "BACKLOG", "QUEUED")
        .expect("Failed to emit state change");

    let event = store
        .get_last_event_of_type("task-003", EventType::StateChanged)
        .expect("Failed to get event")
        .expect("Event not found");

    let data: serde_json::Value = serde_json::from_str(&event.event_data_json).unwrap();
    assert_eq!(data["from"], "BACKLOG");
    assert_eq!(data["to"], "QUEUED");
}
