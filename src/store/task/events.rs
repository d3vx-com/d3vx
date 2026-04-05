//! Task event logging
//!
//! Event logging and retrieval for task audit trails.

use rusqlite::params;

use super::types::TaskLog;
use super::TaskStore;
use crate::store::database::DatabaseError;

impl<'a> TaskStore<'a> {
    /// Log a task event
    pub fn log_event(
        &self,
        task_id: &str,
        phase: &str,
        event: &str,
        data: Option<&serde_json::Value>,
        duration_ms: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();
        let data_json = data
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .unwrap_or_default();

        self.conn
            .execute(
                "INSERT INTO task_logs (task_id, phase, event, data, duration_ms, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![task_id, phase, event, data_json, duration_ms, now],
            )
            .map_err(DatabaseError::QueryError)?;

        Ok(())
    }

    /// Get logs for a task
    pub fn get_logs(
        &self,
        task_id: &str,
        phase: Option<&str>,
    ) -> Result<Vec<TaskLog>, DatabaseError> {
        let logs = if let Some(phase) = phase {
            self.query_logs(
                "SELECT * FROM task_logs WHERE task_id = ?1 AND phase = ?2 ORDER BY created_at ASC",
                params![task_id, phase],
            )?
        } else {
            self.query_logs(
                "SELECT * FROM task_logs WHERE task_id = ?1 ORDER BY created_at ASC",
                params![task_id],
            )?
        };

        Ok(logs)
    }

    /// Helper to query logs
    pub(super) fn query_logs<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<TaskLog>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, TaskLog::from_row)
            .map_err(DatabaseError::QueryError)?;

        let mut logs = Vec::new();
        for row in rows {
            logs.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(logs)
    }
}
