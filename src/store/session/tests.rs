//! Session store tests

use super::store::SessionStore;
use super::types::{NewSession, SessionListOptions, SessionUpdate};
use crate::store::database::Database;

fn create_test_db() -> Database {
    Database::in_memory().expect("Failed to create in-memory database")
}

#[test]
fn test_create_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    let session = store
        .create(NewSession {
            id: None,
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/test/project".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    assert!(session.id.starts_with("ses-"));
    assert_eq!(session.provider, "anthropic");
    assert_eq!(session.model, "claude-3-opus");
    assert_eq!(session.messages, "[]");
    assert_eq!(session.token_count, 0);
}

#[test]
fn test_get_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    let created = store
        .create(NewSession {
            id: Some("test-session".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: None,
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    let fetched = store
        .get("test-session")
        .expect("Failed to get session")
        .expect("Session not found");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.provider, created.provider);
}

#[test]
fn test_get_nonexistent_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    let result = store.get("nonexistent").expect("Query should not error");
    assert!(result.is_none());
}

#[test]
fn test_update_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    store
        .create(NewSession {
            id: Some("test-update".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: None,
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    store
        .update(
            "test-update",
            SessionUpdate {
                messages: Some(r#"[{"role":"user","content":"Hello"}]"#.to_string()),
                token_count: Some(10),
                summary: Some("Test summary".to_string()),
                metadata: None,
                state: None,
            },
        )
        .expect("Failed to update session");

    let updated = store
        .get("test-update")
        .expect("Failed to get session")
        .expect("Session not found");

    assert_eq!(updated.token_count, 10);
    assert_eq!(updated.summary, Some("Test summary".to_string()));
}

#[test]
fn test_delete_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    store
        .create(NewSession {
            id: Some("test-delete".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: None,
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    store
        .delete("test-delete")
        .expect("Failed to delete session");

    let result = store.get("test-delete").expect("Query should not error");
    assert!(result.is_none());
}

#[test]
fn test_list_sessions() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    // Create multiple sessions
    for i in 0..5 {
        store
            .create(NewSession {
                id: Some(format!("session-{}", i)),
                task_id: None,
                provider: "anthropic".to_string(),
                model: "claude-3-opus".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: Some("/test/project".to_string()),
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .expect("Failed to create session");
    }

    let sessions = store
        .list(SessionListOptions {
            project_path: Some("/test/project".to_string()),
            limit: Some(10),
            offset: None,
            task_id: None,
        })
        .expect("Failed to list sessions");

    assert_eq!(sessions.len(), 5);
}

#[test]
fn test_count_sessions() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    for i in 0..3 {
        store
            .create(NewSession {
                id: Some(format!("count-session-{}", i)),
                task_id: None,
                provider: "anthropic".to_string(),
                model: "claude-3-opus".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: Some("/count/project".to_string()),
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .expect("Failed to create session");
    }

    let count = store
        .count(Some("/count/project"))
        .expect("Failed to count sessions");
    assert_eq!(count, 3);

    let total = store.count(None).expect("Failed to count all sessions");
    assert!(total >= 3);
}

#[test]
fn test_get_latest_session() {
    let db = create_test_db();
    let store = SessionStore::new(&db);

    store
        .create(NewSession {
            id: Some("first".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/latest/test".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create first session");

    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(10));

    store
        .create(NewSession {
            id: Some("second".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/latest/test".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create second session");

    let latest = store
        .get_latest(Some("/latest/test"))
        .expect("Failed to get latest")
        .expect("No latest session");

    assert_eq!(latest.id, "second");
}
