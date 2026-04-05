//! Event store CRUD operations
//!
//! Database operations for the append-only event log, plus
//! helper functions for emitting common event types.

use rusqlite::{params, Connection, Row};
use tracing::debug;

use super::types::{EventListOptions, EventType, NewEvent, TaskEvent};
use crate::store::database::{Database, DatabaseError};

/// Event store for append-only logging
pub struct EventStore<'a> {
    conn: &'a Connection,
}

impl<'a> EventStore<'a> {
    /// Create a new event store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new event store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Append a new event
    pub fn append(&self, input: NewEvent) -> Result<TaskEvent, DatabaseError> {
        let now = crate::store::now_iso();

        let event_data_json = serde_json::to_string(&input.data.unwrap_or(serde_json::json!({})))
            .unwrap_or_else(|_| "{}".to_string());

        self.conn
            .execute(
                "INSERT INTO task_events (task_id, run_id, event_type, event_data, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    input.task_id,
                    input.run_id,
                    input.event_type.to_string(),
                    event_data_json,
                    now,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        let id = self.conn.last_insert_rowid();

        debug!(
            "Event appended: {} (task: {}, type: {:?})",
            id, input.task_id, input.event_type
        );

        Ok(TaskEvent {
            id,
            task_id: input.task_id,
            run_id: input.run_id,
            event_type: input.event_type,
            event_data_json,
            created_at: now,
        })
    }

    /// Get events for a specific task
    pub fn get_for_task(&self, task_id: &str) -> Result<Vec<TaskEvent>, DatabaseError> {
        self.query_events(
            "SELECT * FROM task_events WHERE task_id = ?1 ORDER BY id ASC",
            params![task_id],
        )
    }

    /// Get events for a specific run
    pub fn get_for_run(&self, run_id: &str) -> Result<Vec<TaskEvent>, DatabaseError> {
        self.query_events(
            "SELECT * FROM task_events WHERE run_id = ?1 ORDER BY id ASC",
            params![run_id],
        )
    }

    /// Get recent events (all tasks)
    pub fn get_recent(&self, limit: usize) -> Result<Vec<TaskEvent>, DatabaseError> {
        self.query_events(
            "SELECT * FROM task_events ORDER BY id DESC LIMIT ?1",
            params![limit as i64],
        )
    }

    /// List events with filtering options
    pub fn list(&self, options: EventListOptions) -> Result<Vec<TaskEvent>, DatabaseError> {
        let limit = options.limit.unwrap_or(100);
        let offset = options.offset.unwrap_or(0);
        let order = if options.descending { "DESC" } else { "ASC" };

        let events = if let Some(task_id) = &options.task_id {
            let sql = format!(
                "SELECT * FROM task_events WHERE task_id = ?1 ORDER BY id {} LIMIT ?2 OFFSET ?3",
                order
            );
            self.query_events(&sql, params![task_id, limit as i64, offset as i64])?
        } else if let Some(run_id) = &options.run_id {
            let sql = format!(
                "SELECT * FROM task_events WHERE run_id = ?1 ORDER BY id {} LIMIT ?2 OFFSET ?3",
                order
            );
            self.query_events(&sql, params![run_id, limit as i64, offset as i64])?
        } else if let Some(event_types) = &options.event_type {
            let placeholders: Vec<String> = event_types.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT * FROM task_events WHERE event_type IN ({}) ORDER BY id {} LIMIT ? OFFSET ?",
                placeholders.join(","),
                order
            );
            let mut params: Vec<String> = event_types.iter().map(|t| t.to_string()).collect();
            params.push(limit.to_string());
            params.push(offset.to_string());
            self.query_events_with_params(&sql, &params)?
        } else {
            let sql = format!(
                "SELECT * FROM task_events ORDER BY id {} LIMIT ?1 OFFSET ?2",
                order
            );
            self.query_events(&sql, params![limit as i64, offset as i64])?
        };

        Ok(events)
    }

    /// Get the last event of a specific type for a task
    pub fn get_last_event_of_type(
        &self,
        task_id: &str,
        event_type: EventType,
    ) -> Result<Option<TaskEvent>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM task_events WHERE task_id = ?1 AND event_type = ?2 ORDER BY id DESC LIMIT 1",
            params![task_id, event_type.to_string()],
            Self::row_to_event,
        );

        match result {
            Ok(event) => Ok(Some(event)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Count events for a task
    pub fn count_for_task(&self, task_id: &str) -> Result<i64, DatabaseError> {
        let count = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(DatabaseError::QueryError)?;

        Ok(count)
    }

    /// Helper to query events
    fn query_events<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<TaskEvent>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Self::row_to_event)
            .map_err(DatabaseError::QueryError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(events)
    }

    /// Helper to query events with string params
    fn query_events_with_params(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<TaskEvent>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), Self::row_to_event)
            .map_err(DatabaseError::QueryError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(events)
    }

    /// Map a database row to a TaskEvent
    fn row_to_event(row: &Row<'_>) -> rusqlite::Result<TaskEvent> {
        let event_type_str: String = row.get("event_type")?;

        Ok(TaskEvent {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            run_id: row.get("run_id")?,
            event_type: event_type_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            event_data_json: row.get("event_data")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Helper function to emit a state changed event
pub fn emit_state_change(
    store: &EventStore,
    task_id: &str,
    from_state: &str,
    to_state: &str,
) -> Result<(), DatabaseError> {
    store.append(NewEvent {
        task_id: task_id.to_string(),
        run_id: None,
        event_type: EventType::StateChanged,
        data: Some(serde_json::json!({
            "from": from_state,
            "to": to_state,
        })),
    })?;
    Ok(())
}

/// Helper function to emit a worker assigned event
pub fn emit_worker_assigned(
    store: &EventStore,
    task_id: &str,
    run_id: &str,
    worker_id: &str,
) -> Result<(), DatabaseError> {
    store.append(NewEvent {
        task_id: task_id.to_string(),
        run_id: Some(run_id.to_string()),
        event_type: EventType::WorkerAssigned,
        data: Some(serde_json::json!({
            "worker_id": worker_id,
        })),
    })?;
    Ok(())
}
