//! Tool execution audit store
//!
//! Writes to the `tool_executions` table. The table ships in schema V1
//! (see `migrations.rs`) but previously had no Rust writer — tool
//! invocations were invisible to the dashboard and could not be
//! attributed across multi-agent sessions. This store closes that gap.
//!
//! Writes are best-effort from the agent loop: recording failures do
//! not fail tool execution itself, since the audit trail is a
//! diagnostic/observability signal, not a correctness dependency.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::database::{Database, DatabaseError};

/// A persisted tool execution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub id: i64,
    pub session_id: String,
    pub tool_name: String,
    /// JSON-serialised tool input.
    pub tool_input: String,
    /// Tool output text, or `None` if the tool produced no content.
    pub tool_result: Option<String>,
    pub is_error: bool,
    /// Execution time in milliseconds, or `None` if not measured.
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

/// Input for recording a new tool execution.
///
/// `tool_input` is a JSON value; it will be stringified before storage.
/// `duration_ms` is accepted as `u64` for ergonomic call sites and
/// converted to `i64` when written.
#[derive(Debug, Clone)]
pub struct NewToolExecution {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_result: Option<String>,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}

/// CRUD surface for the `tool_executions` table.
pub struct ToolExecutionStore<'a> {
    conn: &'a Connection,
}

impl<'a> ToolExecutionStore<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Append a tool execution record. Returns the persisted record
    /// including the auto-assigned id and timestamp.
    pub fn record(
        &self,
        input: NewToolExecution,
    ) -> Result<ToolExecutionRecord, DatabaseError> {
        let now = crate::store::now_iso();
        let tool_input_json =
            serde_json::to_string(&input.tool_input).unwrap_or_else(|_| "{}".to_string());
        let is_error_int = if input.is_error { 1i64 } else { 0i64 };
        let duration_i64 = input.duration_ms.map(|d| d as i64);

        self.conn
            .execute(
                "INSERT INTO tool_executions \
                 (session_id, tool_name, tool_input, tool_result, is_error, duration_ms, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    input.session_id,
                    input.tool_name,
                    tool_input_json,
                    input.tool_result,
                    is_error_int,
                    duration_i64,
                    now,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        let id = self.conn.last_insert_rowid();
        debug!(
            id,
            session = %input.session_id,
            tool = %input.tool_name,
            is_error = input.is_error,
            "tool execution recorded"
        );

        Ok(ToolExecutionRecord {
            id,
            session_id: input.session_id,
            tool_name: input.tool_name,
            tool_input: tool_input_json,
            tool_result: input.tool_result,
            is_error: input.is_error,
            duration_ms: duration_i64,
            created_at: now,
        })
    }

    /// List all tool executions for a session, oldest first.
    pub fn list_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<ToolExecutionRecord>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, tool_name, tool_input, tool_result, is_error, \
                        duration_ms, created_at \
                 FROM tool_executions WHERE session_id = ?1 ORDER BY id ASC",
            )
            .map_err(DatabaseError::QueryError)?;

        let rows = stmt
            .query_map(params![session_id], Self::row_to_record)
            .map_err(DatabaseError::QueryError)?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(DatabaseError::QueryError)?);
        }
        Ok(records)
    }

    /// Count tool executions for a session.
    pub fn count_for_session(&self, session_id: &str) -> Result<i64, DatabaseError> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM tool_executions WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(DatabaseError::QueryError)
    }

    /// List most recent tool executions across all sessions (for the
    /// dashboard's "live activity" feed).
    pub fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<ToolExecutionRecord>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, tool_name, tool_input, tool_result, is_error, \
                        duration_ms, created_at \
                 FROM tool_executions ORDER BY id DESC LIMIT ?1",
            )
            .map_err(DatabaseError::QueryError)?;

        let rows = stmt
            .query_map(params![limit as i64], Self::row_to_record)
            .map_err(DatabaseError::QueryError)?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(DatabaseError::QueryError)?);
        }
        Ok(records)
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<ToolExecutionRecord> {
        let is_error_int: i64 = row.get(5)?;
        Ok(ToolExecutionRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            tool_name: row.get(2)?,
            tool_input: row.get(3)?,
            tool_result: row.get(4)?,
            is_error: is_error_int != 0,
            duration_ms: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}
