//! Session store CRUD operations
//!
//! Database operations for creating, reading, updating, and
//! deleting conversation sessions.

use rusqlite::{params, Connection, Row};
use tracing::{debug, warn};

use super::types::{NewSession, Session, SessionListOptions, SessionUpdate};
use crate::store::database::{Database, DatabaseError};

/// Session store for CRUD operations
pub struct SessionStore<'a> {
    conn: &'a Connection,
}

impl<'a> SessionStore<'a> {
    /// Create a new session store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new session store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new session
    pub fn create(&self, input: NewSession) -> Result<Session, DatabaseError> {
        let now = crate::store::now_iso();
        let id = input.id.unwrap_or_else(|| crate::store::generate_id("ses"));

        let session = Session {
            id: id.clone(),
            task_id: input.task_id,
            provider: input.provider,
            model: input.model,
            messages: input.messages.unwrap_or_else(|| "[]".to_string()),
            token_count: input.token_count.unwrap_or(0),
            summary: input.summary,
            project_path: input.project_path,
            parent_session_id: input.parent_session_id,
            created_at: now.clone(),
            updated_at: now,
            metadata: input.metadata.unwrap_or_else(|| "{}".to_string()),
            state: input.state.unwrap_or_default(),
        };

        self.conn
            .execute(
                "INSERT INTO sessions (
                    id, task_id, provider, model, messages, token_count, summary,
                    project_path, parent_session_id, created_at, updated_at, metadata, state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    session.id,
                    session.task_id,
                    session.provider,
                    session.model,
                    session.messages,
                    session.token_count,
                    session.summary,
                    session.project_path,
                    session.parent_session_id,
                    session.created_at,
                    session.updated_at,
                    session.metadata,
                    session.state.to_string(),
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Session created: {}", id);
        Ok(session)
    }

    /// Get a session by ID
    pub fn get(&self, id: &str) -> Result<Option<Session>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            params![id],
            Self::row_to_session,
        );

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Update a session
    pub fn update(&self, id: &str, updates: SessionUpdate) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        let rows_affected = self
            .conn
            .execute(
                "UPDATE sessions SET
                    messages = COALESCE(?1, messages),
                    token_count = COALESCE(?2, token_count),
                    summary = COALESCE(?3, summary),
                    metadata = COALESCE(?4, metadata),
                    state = COALESCE(?5, state),
                    updated_at = ?6
                WHERE id = ?7",
                params![
                    updates.messages,
                    updates.token_count,
                    updates.summary,
                    updates.metadata,
                    updates.state.map(|s| s.to_string()),
                    now,
                    id,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        if rows_affected == 0 {
            warn!("Attempted to update non-existent session: {}", id);
        } else {
            debug!("Session updated: {}", id);
        }

        Ok(())
    }

    /// Delete a session
    pub fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])
            .map_err(DatabaseError::QueryError)?;

        debug!("Session deleted: {}", id);
        Ok(())
    }

    /// List sessions with optional filtering
    pub fn list(&self, options: SessionListOptions) -> Result<Vec<Session>, DatabaseError> {
        let limit = options.limit.unwrap_or(50);
        let offset = options.offset.unwrap_or(0);

        let sessions = if let Some(task_id) = &options.task_id {
            self.query_sessions(
                "SELECT * FROM sessions WHERE task_id = ?1 ORDER BY updated_at DESC",
                params![task_id],
            )?
        } else if let Some(project_path) = &options.project_path {
            self.query_sessions(
                "SELECT * FROM sessions WHERE project_path = ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3",
                params![project_path, limit as i64, offset as i64],
            )?
        } else {
            self.query_sessions(
                "SELECT * FROM sessions ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
                params![limit as i64, offset as i64],
            )?
        };

        Ok(sessions)
    }

    /// Get the most recent session (optionally filtered by project)
    pub fn get_latest(&self, project_path: Option<&str>) -> Result<Option<Session>, DatabaseError> {
        let result = if let Some(path) = project_path {
            self.conn.query_row(
                "SELECT * FROM sessions WHERE project_path = ?1 ORDER BY updated_at DESC LIMIT 1",
                params![path],
                Self::row_to_session,
            )
        } else {
            self.conn.query_row(
                "SELECT * FROM sessions ORDER BY updated_at DESC LIMIT 1",
                [],
                Self::row_to_session,
            )
        };

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Get session count
    pub fn count(&self, project_path: Option<&str>) -> Result<i64, DatabaseError> {
        let count = if let Some(path) = project_path {
            self.conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE project_path = ?1",
                params![path],
                |row| row.get(0),
            )
        } else {
            self.conn
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        }
        .map_err(DatabaseError::QueryError)?;

        Ok(count)
    }

    /// Helper to query sessions with params
    fn query_sessions<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<Session>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Self::row_to_session)
            .map_err(DatabaseError::QueryError)?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(sessions)
    }

    /// Map a database row to a Session struct
    fn row_to_session(row: &Row<'_>) -> rusqlite::Result<Session> {
        Ok(Session {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            messages: row.get("messages")?,
            token_count: row.get("token_count")?,
            summary: row.get("summary")?,
            project_path: row.get("project_path")?,
            parent_session_id: row.get("parent_session_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            metadata: row.get("metadata")?,
            state: row
                .get::<_, String>("state")
                .unwrap_or_else(|_| "SPAWNING".to_string())
                .parse()
                .unwrap_or_default(),
        })
    }
}
