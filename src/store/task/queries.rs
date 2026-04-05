//! Task query methods
//!
//! Listing, fetching next task, retrying, counting,
//! and dependency checks.

use rusqlite::params;
use tracing::{debug, warn};

use super::state_machine::TaskState;
use super::types::{Task, TaskListOptions};
use super::TaskStore;
use crate::store::database::DatabaseError;

impl<'a> TaskStore<'a> {
    /// List tasks with optional filtering
    pub fn list(&self, options: TaskListOptions) -> Result<Vec<Task>, DatabaseError> {
        let limit = options.limit.unwrap_or(100);
        let offset = options.offset.unwrap_or(0);

        let tasks = if let Some(batch_id) = &options.batch_id {
            self.query_tasks(
                "SELECT * FROM tasks WHERE batch_id = ?1 ORDER BY priority DESC, created_at ASC",
                params![batch_id],
            )?
        } else if let Some(states) = &options.state {
            let placeholders: Vec<String> = states.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT * FROM tasks WHERE state IN ({}) ORDER BY priority DESC, created_at ASC",
                placeholders.join(",")
            );
            let sql_params: Vec<String> = states.iter().map(|s| s.to_string()).collect();
            self.query_tasks_with_params(&sql, &sql_params)?
        } else if let Some(project_path) = &options.project_path {
            self.query_tasks(
                "SELECT * FROM tasks WHERE project_path = ?1 ORDER BY priority DESC, created_at ASC LIMIT ?2 OFFSET ?3",
                params![project_path, limit as i64, offset as i64],
            )?
        } else {
            self.query_tasks(
                "SELECT * FROM tasks ORDER BY priority DESC, created_at ASC LIMIT ?1 OFFSET ?2",
                params![limit as i64, offset as i64],
            )?
        };

        Ok(tasks)
    }

    /// Get the next task to process (highest priority QUEUED task)
    pub fn get_next(&self) -> Result<Option<Task>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM tasks WHERE state = 'QUEUED' ORDER BY priority DESC, created_at ASC LIMIT 1",
            [],
            Task::from_row,
        );

        match result {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Retry a failed task
    pub fn retry(&self, id: &str) -> Result<bool, DatabaseError> {
        let now = crate::store::now_iso();

        let rows_affected = self.conn
            .execute(
                "UPDATE tasks SET retry_count = retry_count + 1, state = 'QUEUED', error = NULL, updated_at = ?1
                 WHERE id = ?2 AND retry_count < max_retries",
                params![now, id],
            )
            .map_err(DatabaseError::QueryError)?;

        if rows_affected > 0 {
            self.log_event(id, "lifecycle", "retried", None, None)?;
            debug!("Task retried: {}", id);
            Ok(true)
        } else {
            warn!(
                "Task retry failed (max retries reached or not found): {}",
                id
            );
            Ok(false)
        }
    }

    /// Get task counts by state
    pub fn get_counts(&self) -> Result<std::collections::HashMap<TaskState, i64>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare("SELECT state, COUNT(*) as count FROM tasks GROUP BY state")
            .map_err(DatabaseError::QueryError)?;

        let rows = stmt
            .query_map([], |row| {
                let state_str: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((state_str, count))
            })
            .map_err(DatabaseError::QueryError)?;

        let mut counts = std::collections::HashMap::new();
        for row in rows {
            let (state_str, count) = row.map_err(DatabaseError::QueryError)?;
            if let Ok(state) = state_str.parse::<TaskState>() {
                counts.insert(state, count);
            }
        }

        Ok(counts)
    }

    /// Check if a task's dependencies are satisfied
    pub fn are_dependencies_met(&self, id: &str) -> Result<bool, DatabaseError> {
        let task = match self.get(id)? {
            Some(t) => t,
            None => return Ok(false),
        };

        let deps: Vec<String> = serde_json::from_str(&task.depends_on).unwrap_or_default();

        if deps.is_empty() {
            return Ok(true);
        }

        for dep_id in deps {
            let dep = match self.get(&dep_id)? {
                Some(d) => d,
                None => return Ok(false),
            };
            if dep.state != TaskState::Done {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
