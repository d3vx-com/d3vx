//! Tests for Session Store Operations
//!
//! Covers session CRUD operations and management.

#[cfg(test)]
mod tests {
    use crate::store::database::Database;
    use crate::store::session::{NewSession, SessionListOptions, SessionStore, SessionUpdate};

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    // =========================================================================
    // Session Creation Tests
    // =========================================================================

    #[test]
    fn test_session_creation() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let result = store.create(NewSession {
            id: Some("test-ses".to_string()),
            task_id: None,
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: None,
            parent_session_id: None,
            metadata: None,
            state: None,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_session_with_metadata() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let session = store
            .create(NewSession {
                id: Some("test-metadata".to_string()),
                task_id: None,
                provider: "openai".to_string(),
                model: "gpt-4o".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: Some("/test/project".to_string()),
                parent_session_id: None,
                metadata: Some(r#"{"test":true}"#.to_string()),
                state: None,
            })
            .unwrap();

        let retrieved = store.get(&session.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().model, "gpt-4o");
    }

    // =========================================================================
    // Session Retrieval Tests
    // =========================================================================

    #[test]
    fn test_get_nonexistent_session() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let result = store.get("nonexistent-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_sessions_empty() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let sessions = store.list(SessionListOptions::default()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_sessions_with_data() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        // Create multiple sessions
        for i in 0..3 {
            store
                .create(NewSession {
                    id: Some(format!("ses-{}", i)),
                    task_id: None,
                    provider: "anthropic".to_string(),
                    model: "claude-sonnet-4".to_string(),
                    messages: None,
                    token_count: None,
                    summary: None,
                    project_path: Some(format!("/project/{}", i)),
                    parent_session_id: None,
                    metadata: None,
                    state: None,
                })
                .unwrap();
        }

        let sessions = store.list(SessionListOptions::default()).unwrap();
        assert_eq!(sessions.len(), 3);
    }

    // =========================================================================
    // Session Update Tests
    // =========================================================================

    #[test]
    fn test_update_session() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let id = "test-update".to_string();
        store
            .create(NewSession {
                id: Some(id.clone()),
                task_id: None,
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: None,
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .unwrap();

        // Update the session
        store
            .update(
                &id,
                SessionUpdate {
                    messages: Some("[]".to_string()),
                    token_count: Some(1000),
                    summary: Some("Updated summary".to_string()),
                    metadata: None,
                    state: None,
                },
            )
            .unwrap();

        let retrieved = store.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.token_count, 1000);
        assert_eq!(retrieved.summary, Some("Updated summary".to_string()));
    }

    // =========================================================================
    // Session Deletion Tests
    // =========================================================================

    #[test]
    fn test_delete_session() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        let id = "test-delete".to_string();
        store
            .create(NewSession {
                id: Some(id.clone()),
                task_id: None,
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: None,
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .unwrap();

        store.delete(&id).unwrap();

        let retrieved = store.get(&id).unwrap();
        assert!(retrieved.is_none());
    }

    // =========================================================================
    // Session List Options Tests
    // =========================================================================

    #[test]
    fn test_list_options_limit() {
        let db = create_test_db();
        let store = SessionStore::new(&db);

        // Create 5 sessions
        for i in 0..5 {
            store
                .create(NewSession {
                    id: Some(format!("ses-limit-{}", i)),
                    task_id: None,
                    provider: "anthropic".to_string(),
                    model: "claude-sonnet-4".to_string(),
                    messages: None,
                    token_count: None,
                    summary: None,
                    project_path: Some("/test".to_string()),
                    parent_session_id: None,
                    metadata: None,
                    state: None,
                })
                .unwrap();
        }

        let options = SessionListOptions {
            limit: Some(3),
            ..Default::default()
        };

        let sessions = store.list(options).unwrap();
        assert_eq!(sessions.len(), 3);
    }
}
