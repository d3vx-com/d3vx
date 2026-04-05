//! Task CRUD operations
//!
//! Basic create, read, update, delete operations for tasks,
//! plus the TaskStore struct and helper query methods.

use rusqlite::params;
use tracing::{debug, warn};

use super::enums::ExecutionMode;
use super::state_machine::TaskState;
use super::types::{NewTask, Task, TaskUpdate};
use super::TaskStore;
use crate::store::database::DatabaseError;
use crate::store::workspace::ScopeMode;

impl<'a> TaskStore<'a> {
    /// Create a new task
    pub fn create(&self, input: NewTask) -> Result<Task, DatabaseError> {
        let now = crate::store::now_iso();
        let id = input
            .id
            .unwrap_or_else(|| crate::store::generate_id("task"));

        let depends_on = serde_json::to_string(&input.depends_on.unwrap_or_default())
            .unwrap_or_else(|_| "[]".to_string());
        let metadata = serde_json::to_string(&input.metadata.unwrap_or(serde_json::json!({})))
            .unwrap_or_else(|_| "{}".to_string());

        let state = input.state.unwrap_or(TaskState::Backlog);
        let priority = input.priority.unwrap_or(0);
        let max_retries = input.max_retries.unwrap_or(10);
        let execution_mode = input.execution_mode.unwrap_or(ExecutionMode::Auto);
        let scope_mode = input.scope_mode.unwrap_or(ScopeMode::Repo);

        self.conn
            .execute(
                "INSERT INTO tasks (
                    id, title, description, state, priority, batch_id, max_retries,
                    depends_on, metadata, project_path, agent_role, log_file,
                    execution_mode, repo_root, task_scope_path, scope_mode, parent_task_id,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    id,
                    input.title,
                    input.description,
                    state.to_string(),
                    priority,
                    input.batch_id,
                    max_retries,
                    depends_on,
                    metadata,
                    input.project_path,
                    input.agent_role.map(|r| r.to_string()),
                    Option::<String>::None, // log_file
                    execution_mode.to_string(),
                    input.repo_root,
                    input.task_scope_path,
                    scope_mode.to_string(),
                    input.parent_task_id,
                    now.clone(),
                    now.clone(),
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        // Log creation event
        self.log_event(
            &id,
            "lifecycle",
            "created",
            Some(&serde_json::json!({ "title": input.title })),
            None,
        )?;

        let task = self.get(&id)?.expect("Task should exist after creation");
        debug!("Task created: {} ({})", task.id, task.title);
        Ok(task)
    }

    /// Get a task by ID
    pub fn get(&self, id: &str) -> Result<Option<Task>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM tasks WHERE id = ?1",
            params![id],
            Task::from_row,
        );

        match result {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Transition a task to a new state
    pub fn transition(&self, id: &str, new_state: TaskState) -> Result<(), DatabaseError> {
        let task = self
            .get(id)?
            .ok_or_else(|| DatabaseError::QueryError(rusqlite::Error::InvalidQuery))?;

        if !task.state.can_transition_to(new_state) {
            let valid = task.state.valid_transitions();
            warn!(
                "Invalid state transition: {} -> {} (allowed: {:?})",
                task.state, new_state, valid
            );
            return Err(DatabaseError::QueryError(rusqlite::Error::InvalidQuery));
        }

        let now = crate::store::now_iso();
        let new_state_str = new_state.to_string();

        self.conn
            .execute(
                "UPDATE tasks SET
                    state = ?1,
                    pipeline_phase = ?2,
                    updated_at = ?3,
                    started_at = CASE
                        WHEN ?1 IN ('PREPARING', 'SPAWNING', 'RESEARCH') AND started_at IS NULL
                        THEN ?3
                        ELSE started_at
                    END,
                    completed_at = CASE
                        WHEN ?1 IN ('DONE', 'FAILED')
                        THEN ?3
                        ELSE completed_at
                    END
                WHERE id = ?4",
                params![new_state_str, new_state_str, now, id],
            )
            .map_err(DatabaseError::QueryError)?;

        self.log_event(
            id,
            "lifecycle",
            "state_changed",
            Some(&serde_json::json!({
                "from": task.state.to_string(),
                "to": new_state.to_string()
            })),
            None,
        )?;

        debug!("Task state changed: {} -> {}", id, new_state);
        Ok(())
    }

    /// Update task fields
    pub fn update(&self, id: &str, updates: TaskUpdate) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        let metadata_json = updates
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        self.conn
            .execute(
                "UPDATE tasks SET
                    title = COALESCE(?1, title),
                    description = COALESCE(?2, description),
                    state = COALESCE(?3, state),
                    pipeline_phase = COALESCE(?4, pipeline_phase),
                    priority = COALESCE(?5, priority),
                    worktree_path = COALESCE(?6, worktree_path),
                    worktree_branch = COALESCE(?7, worktree_branch),
                    checkpoint_data = COALESCE(?8, checkpoint_data),
                    error = ?9,
                    metadata = COALESCE(?10, metadata),
                    agent_role = COALESCE(?11, agent_role),
                    log_file = COALESCE(?12, log_file),
                    updated_at = ?13
                WHERE id = ?14",
                params![
                    updates.title,
                    updates.description,
                    updates.state.map(|s| s.to_string()),
                    updates.pipeline_phase,
                    updates.priority,
                    updates.worktree_path,
                    updates.worktree_branch,
                    updates.checkpoint_data,
                    updates.error,
                    metadata_json,
                    updates.agent_role.map(|r| r.to_string()),
                    updates.log_file,
                    now,
                    id,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Task updated: {}", id);
        Ok(())
    }

    /// Delete a task
    pub fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .map_err(DatabaseError::QueryError)?;

        debug!("Task deleted: {}", id);
        Ok(())
    }

    /// Helper to query tasks
    pub(super) fn query_tasks<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<Task>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Task::from_row)
            .map_err(DatabaseError::QueryError)?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(tasks)
    }

    /// Helper to query tasks with string params
    pub(super) fn query_tasks_with_params(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<Task>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), Task::from_row)
            .map_err(DatabaseError::QueryError)?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(tasks)
    }
}
