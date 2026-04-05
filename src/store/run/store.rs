//! Task run store CRUD operations
//!
//! Database operations for creating, reading, updating, and
//! deleting task run records.

use rusqlite::{params, Connection, Row};
use tracing::debug;

use super::types::{NewTaskRun, RunStatus, TaskRun, TaskRunListOptions, TaskRunUpdate};
use crate::store::database::{Database, DatabaseError};

/// Task run store for CRUD operations
pub struct TaskRunStore<'a> {
    conn: &'a Connection,
}

impl<'a> TaskRunStore<'a> {
    /// Create a new task run store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new task run store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new task run
    pub fn create(&self, input: NewTaskRun) -> Result<TaskRun, DatabaseError> {
        let now = crate::store::now_iso();
        let id = input.id.unwrap_or_else(|| crate::store::generate_id("run"));

        let metrics_json = serde_json::to_string(&input.metrics.unwrap_or(serde_json::json!({})))
            .unwrap_or_else(|_| "{}".to_string());

        let run = TaskRun {
            id: id.clone(),
            task_id: input.task_id.clone(),
            attempt_number: input.attempt_number,
            status: RunStatus::Pending,
            worker_id: input.worker_id,
            workspace_id: input.workspace_id,
            started_at: Some(now.clone()),
            ended_at: None,
            failure_reason: None,
            summary: None,
            metrics_json,
        };

        self.conn
            .execute(
                "INSERT INTO task_runs (
                    id, task_id, attempt_number, status, worker_id, workspace_id,
                    started_at, ended_at, failure_reason, summary, metrics_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    run.id,
                    run.task_id,
                    run.attempt_number,
                    run.status.to_string(),
                    run.worker_id,
                    run.workspace_id,
                    run.started_at,
                    run.ended_at,
                    run.failure_reason,
                    run.summary,
                    run.metrics_json,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!(
            "Task run created: {} (task: {}, attempt: {})",
            id, input.task_id, input.attempt_number
        );
        Ok(run)
    }

    /// Get a task run by ID
    pub fn get(&self, id: &str) -> Result<Option<TaskRun>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM task_runs WHERE id = ?1",
            params![id],
            Self::row_to_run,
        );

        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Update a task run
    pub fn update(&self, id: &str, updates: TaskRunUpdate) -> Result<(), DatabaseError> {
        let metrics_json = updates
            .metrics
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        let rows_affected = self
            .conn
            .execute(
                "UPDATE task_runs SET
                    status = COALESCE(?1, status),
                    worker_id = ?2,
                    workspace_id = ?3,
                    ended_at = COALESCE(?4, ended_at),
                    failure_reason = ?5,
                    summary = ?6,
                    metrics_json = COALESCE(?7, metrics_json)
                WHERE id = ?8",
                params![
                    updates.status.map(|s| s.to_string()),
                    updates.worker_id,
                    updates.workspace_id,
                    updates.ended_at,
                    updates.failure_reason,
                    updates.summary,
                    metrics_json,
                    id,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        if rows_affected == 0 {
            debug!("No run found to update: {}", id);
        } else {
            debug!("Task run updated: {}", id);
        }

        Ok(())
    }

    /// Update run status
    pub fn update_status(&self, id: &str, status: RunStatus) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        let ended_at = match status {
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled => Some(now),
            _ => None,
        };

        self.conn
            .execute(
                "UPDATE task_runs SET status = ?1, ended_at = COALESCE(?2, ended_at) WHERE id = ?3",
                params![status.to_string(), ended_at, id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Task run status updated: {} -> {}", id, status);
        Ok(())
    }

    /// List runs for a specific task
    pub fn list_runs_for_task(&self, task_id: &str) -> Result<Vec<TaskRun>, DatabaseError> {
        self.query_runs(
            "SELECT * FROM task_runs WHERE task_id = ?1 ORDER BY attempt_number DESC",
            params![task_id],
        )
    }

    /// Get all active (running) runs
    pub fn get_active_runs(&self) -> Result<Vec<TaskRun>, DatabaseError> {
        self.query_runs(
            "SELECT * FROM task_runs WHERE status = 'RUNNING' ORDER BY started_at ASC",
            params![],
        )
    }

    /// List runs with filtering options
    pub fn list(&self, options: TaskRunListOptions) -> Result<Vec<TaskRun>, DatabaseError> {
        let limit = options.limit.unwrap_or(100);
        let offset = options.offset.unwrap_or(0);

        let runs = if let Some(task_id) = &options.task_id {
            self.query_runs(
                "SELECT * FROM task_runs WHERE task_id = ?1 ORDER BY attempt_number DESC LIMIT ?2 OFFSET ?3",
                params![task_id, limit as i64, offset as i64],
            )?
        } else if let Some(worker_id) = &options.worker_id {
            self.query_runs(
                "SELECT * FROM task_runs WHERE worker_id = ?1 ORDER BY started_at DESC LIMIT ?2 OFFSET ?3",
                params![worker_id, limit as i64, offset as i64],
            )?
        } else if let Some(statuses) = &options.status {
            let placeholders: Vec<String> = statuses.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT * FROM task_runs WHERE status IN ({}) ORDER BY started_at DESC LIMIT ? OFFSET ?",
                placeholders.join(",")
            );
            let mut params: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
            params.push(limit.to_string());
            params.push(offset.to_string());
            self.query_runs_with_params(&sql, &params)?
        } else {
            self.query_runs(
                "SELECT * FROM task_runs ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
                params![limit as i64, offset as i64],
            )?
        };

        Ok(runs)
    }

    /// Get the latest run for a task
    pub fn get_latest_for_task(&self, task_id: &str) -> Result<Option<TaskRun>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM task_runs WHERE task_id = ?1 ORDER BY attempt_number DESC LIMIT 1",
            params![task_id],
            Self::row_to_run,
        );

        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Get the next attempt number for a task
    pub fn get_next_attempt_number(&self, task_id: &str) -> Result<i32, DatabaseError> {
        let max_attempt: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(attempt_number), 0) FROM task_runs WHERE task_id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(DatabaseError::QueryError)?;

        Ok(max_attempt + 1)
    }

    /// Helper to query runs
    fn query_runs<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<TaskRun>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Self::row_to_run)
            .map_err(DatabaseError::QueryError)?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(runs)
    }

    /// Helper to query runs with string params
    fn query_runs_with_params(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<TaskRun>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), Self::row_to_run)
            .map_err(DatabaseError::QueryError)?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(runs)
    }

    /// Map a database row to a TaskRun
    fn row_to_run(row: &Row<'_>) -> rusqlite::Result<TaskRun> {
        let status_str: String = row.get("status")?;

        Ok(TaskRun {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            attempt_number: row.get("attempt_number")?,
            status: status_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            worker_id: row.get("worker_id")?,
            workspace_id: row.get("workspace_id")?,
            started_at: row.get("started_at")?,
            ended_at: row.get("ended_at")?,
            failure_reason: row.get("failure_reason")?,
            summary: row.get("summary")?,
            metrics_json: row.get("metrics_json")?,
        })
    }
}
