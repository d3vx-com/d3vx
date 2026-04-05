//! Message store for session messages
//!
//! Stores individual messages for each session, supporting
//! content blocks (text, tool_use, tool_result).

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::database::{Database, DatabaseError};

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for MessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            _ => Err(format!("Invalid message role: {}", s)),
        }
    }
}

/// Content type for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Blocks,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Text => write!(f, "text"),
            ContentType::Blocks => write!(f, "blocks"),
        }
    }
}

/// A message record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    /// Record ID (auto-increment)
    pub id: i64,
    /// Session this message belongs to
    pub session_id: String,
    /// Message role (user/assistant/system)
    pub role: MessageRole,
    /// Message content (text or JSON for blocks)
    pub content: String,
    /// Content type
    pub content_type: ContentType,
    /// Token count for this message
    pub token_count: i64,
    /// Creation timestamp
    pub created_at: String,
}

/// Input for adding a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessage {
    /// Session ID
    pub session_id: String,
    /// Message role
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Content type
    pub content_type: Option<ContentType>,
    /// Token count
    pub token_count: Option<i64>,
}

/// Message store for CRUD operations
pub struct MessageStore<'a> {
    conn: &'a Connection,
}

impl<'a> MessageStore<'a> {
    /// Create a new message store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new message store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Add a message to a session
    pub fn add(&self, input: NewMessage) -> Result<MessageRecord, DatabaseError> {
        let now = crate::store::now_iso();
        let content_type = input.content_type.unwrap_or(ContentType::Text);
        let token_count = input.token_count.unwrap_or(0);

        self.conn
            .execute(
                "INSERT INTO messages (session_id, role, content, content_type, token_count, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    input.session_id,
                    input.role.to_string(),
                    input.content,
                    content_type.to_string(),
                    token_count,
                    now,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        let id = self.conn.last_insert_rowid();

        debug!("Message added: id={}, session={}", id, input.session_id);

        Ok(MessageRecord {
            id,
            session_id: input.session_id,
            role: input.role,
            content: input.content,
            content_type,
            token_count,
            created_at: now,
        })
    }

    /// Get all messages for a session
    pub fn get_for_session(&self, session_id: &str) -> Result<Vec<MessageRecord>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at ASC")
            .map_err(DatabaseError::QueryError)?;

        let rows = stmt
            .query_map(params![session_id], Self::row_to_message)
            .map_err(DatabaseError::QueryError)?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(messages)
    }

    /// Get messages for a session with pagination
    pub fn get_for_session_paginated(
        &self,
        session_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MessageRecord>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at ASC LIMIT ?2 OFFSET ?3",
            )
            .map_err(DatabaseError::QueryError)?;

        let rows = stmt
            .query_map(
                params![session_id, limit as i64, offset as i64],
                Self::row_to_message,
            )
            .map_err(DatabaseError::QueryError)?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(messages)
    }

    /// Count messages for a session
    pub fn count_for_session(&self, session_id: &str) -> Result<i64, DatabaseError> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(DatabaseError::QueryError)
    }

    /// Delete all messages for a session
    pub fn delete_for_session(&self, session_id: &str) -> Result<usize, DatabaseError> {
        let count = self
            .conn
            .execute(
                "DELETE FROM messages WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Deleted {} messages for session {}", count, session_id);
        Ok(count)
    }

    /// Delete a specific message
    pub fn delete(&self, id: i64) -> Result<bool, DatabaseError> {
        let count = self
            .conn
            .execute("DELETE FROM messages WHERE id = ?1", params![id])
            .map_err(DatabaseError::QueryError)?;

        Ok(count > 0)
    }

    /// Get a message by ID
    pub fn get(&self, id: i64) -> Result<Option<MessageRecord>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM messages WHERE id = ?1",
            params![id],
            Self::row_to_message,
        );

        match result {
            Ok(message) => Ok(Some(message)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Map a database row to a MessageRecord
    fn row_to_message(row: &Row<'_>) -> rusqlite::Result<MessageRecord> {
        let role_str: String = row.get("role")?;
        let content_type_str: String = row.get("content_type")?;

        Ok(MessageRecord {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            role: role_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            content: row.get("content")?,
            content_type: match content_type_str.as_str() {
                "blocks" => ContentType::Blocks,
                _ => ContentType::Text,
            },
            token_count: row.get("token_count")?,
            created_at: row.get("created_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::session::{NewSession, SessionStore};
    use std::str::FromStr;

    fn create_test_db() -> Database {
        Database::in_memory().expect("Failed to create in-memory database")
    }

    fn create_test_session(db: &Database) -> String {
        let session_store = SessionStore::new(db);
        let session = session_store
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
        session.id
    }

    #[test]
    fn test_add_message() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        let message = store
            .add(NewMessage {
                session_id: session_id.clone(),
                role: MessageRole::User,
                content: "Hello, world!".to_string(),
                content_type: None,
                token_count: Some(3),
            })
            .expect("Failed to add message");

        assert!(message.id > 0);
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello, world!");
        assert_eq!(message.content_type, ContentType::Text);
        assert_eq!(message.token_count, 3);
    }

    #[test]
    fn test_get_messages_for_session() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        // Add multiple messages
        store
            .add(NewMessage {
                session_id: session_id.clone(),
                role: MessageRole::User,
                content: "First".to_string(),
                content_type: None,
                token_count: None,
            })
            .expect("Failed to add first message");

        store
            .add(NewMessage {
                session_id: session_id.clone(),
                role: MessageRole::Assistant,
                content: "Second".to_string(),
                content_type: None,
                token_count: None,
            })
            .expect("Failed to add second message");

        let messages = store
            .get_for_session(&session_id)
            .expect("Failed to get messages");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_count_messages() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        for i in 0..5 {
            store
                .add(NewMessage {
                    session_id: session_id.clone(),
                    role: MessageRole::User,
                    content: format!("Message {}", i),
                    content_type: None,
                    token_count: None,
                })
                .expect("Failed to add message");
        }

        let count = store
            .count_for_session(&session_id)
            .expect("Failed to count messages");
        assert_eq!(count, 5);
    }

    #[test]
    fn test_delete_messages() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        store
            .add(NewMessage {
                session_id: session_id.clone(),
                role: MessageRole::User,
                content: "To be deleted".to_string(),
                content_type: None,
                token_count: None,
            })
            .expect("Failed to add message");

        let count = store
            .delete_for_session(&session_id)
            .expect("Failed to delete messages");
        assert_eq!(count, 1);

        let remaining = store
            .get_for_session(&session_id)
            .expect("Failed to get messages");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_message_role_parse() {
        assert_eq!(MessageRole::from_str("user").unwrap(), MessageRole::User);
        assert_eq!(
            MessageRole::from_str("assistant").unwrap(),
            MessageRole::Assistant
        );
        assert_eq!(
            MessageRole::from_str("system").unwrap(),
            MessageRole::System
        );
        assert!(MessageRole::from_str("invalid").is_err());
    }

    #[test]
    fn test_paginated_messages() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = MessageStore::new(&db);

        // Add 10 messages
        for i in 0..10 {
            store
                .add(NewMessage {
                    session_id: session_id.clone(),
                    role: MessageRole::User,
                    content: format!("Message {}", i),
                    content_type: None,
                    token_count: None,
                })
                .expect("Failed to add message");
        }

        let page1 = store
            .get_for_session_paginated(&session_id, 5, 0)
            .expect("Failed to get page 1");
        assert_eq!(page1.len(), 5);

        let page2 = store
            .get_for_session_paginated(&session_id, 5, 5)
            .expect("Failed to get page 2");
        assert_eq!(page2.len(), 5);
    }
}
