//! Tests for Message Store Operations
//!
//! Covers message CRUD operations for conversation history.

#[cfg(test)]
mod tests {
    use crate::store::database::Database;
    use crate::store::message::{ContentType, MessageRole, MessageStore, NewMessage};
    use crate::store::session::{NewSession, SessionStore};

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn create_test_session(db: &Database) -> String {
        let store = SessionStore::new(db);
        let session = store
            .create(NewSession {
                id: None,
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
        session.id
    }

    // =========================================================================
    // Message Creation Tests
    // =========================================================================

    #[test]
    fn test_message_creation() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        let result = store.add(NewMessage {
            session_id: session_id.clone(),
            role: MessageRole::User,
            content: "Hello, assistant!".to_string(),
            content_type: Some(ContentType::Text),
            token_count: Some(10),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_assistant_message_creation() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        let result = store.add(NewMessage {
            session_id: session_id.clone(),
            role: MessageRole::Assistant,
            content: "Hello! How can I help you?".to_string(),
            content_type: Some(ContentType::Text),
            token_count: Some(20),
        });
        assert!(result.is_ok());
    }

    // =========================================================================
    // Message Retrieval Tests
    // =========================================================================

    #[test]
    fn test_get_session_messages() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        // Create messages
        for i in 0..3 {
            store
                .add(NewMessage {
                    session_id: session_id.clone(),
                    role: if i % 2 == 0 {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    },
                    content: format!("Message {}", i),
                    content_type: Some(ContentType::Text),
                    token_count: Some(10),
                })
                .unwrap();
        }

        let messages = store.get_for_session(&session_id).unwrap();
        assert_eq!(messages.len(), 3);
    }

    // =========================================================================
    // Message Role Tests
    // =========================================================================

    #[test]
    fn test_message_role_user() {
        let role = MessageRole::User;
        assert_eq!(role.to_string(), "user");
    }

    #[test]
    fn test_message_role_assistant() {
        let role = MessageRole::Assistant;
        assert_eq!(role.to_string(), "assistant");
    }

    // =========================================================================
    // Message Order Tests
    // =========================================================================

    #[test]
    fn test_messages_ordered_by_time() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        // Create messages with slight time differences
        for i in 0..3 {
            store
                .add(NewMessage {
                    session_id: session_id.clone(),
                    role: MessageRole::User,
                    content: format!("Message {}", i),
                    content_type: Some(ContentType::Text),
                    token_count: Some(10),
                })
                .unwrap();

            // Small delay to ensure different timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let messages = store.get_for_session(&session_id).unwrap();

        // Verify messages are in order
        for (i, msg) in messages.iter().enumerate() {
            assert_eq!(msg.content, format!("Message {}", i));
        }
    }

    // =========================================================================
    // Message Deletion Tests
    // =========================================================================

    #[test]
    fn test_delete_session_messages() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        // Create messages
        for i in 0..3 {
            store
                .add(NewMessage {
                    session_id: session_id.clone(),
                    role: MessageRole::User,
                    content: format!("Message {}", i),
                    content_type: Some(ContentType::Text),
                    token_count: Some(10),
                })
                .unwrap();
        }

        // Delete all session messages
        store.delete_for_session(&session_id).unwrap();

        let messages = store.get_for_session(&session_id).unwrap();
        assert!(messages.is_empty());
    }
}
