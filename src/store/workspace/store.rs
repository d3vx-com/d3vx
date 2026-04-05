//! Workspace store CRUD operations
//!
//! Database operations for creating, reading, updating, and
//! deleting workspace records.

use rusqlite::{params, Connection, Row};
use tracing::debug;

use super::types::{NewWorkspace, ScopeMode, Workspace, WorkspaceListOptions, WorkspaceStatus};
use crate::store::database::{Database, DatabaseError};

/// Workspace store for CRUD operations
pub struct WorkspaceStore<'a> {
    conn: &'a Connection,
}

impl<'a> WorkspaceStore<'a> {
    /// Create a new workspace store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new workspace store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new workspace
    pub fn create(&self, input: NewWorkspace) -> Result<Workspace, DatabaseError> {
        let now = crate::store::now_iso();
        let id = input.id.unwrap_or_else(|| crate::store::generate_id("ws"));

        let metadata_json = serde_json::to_string(&input.metadata.unwrap_or(serde_json::json!({})))
            .unwrap_or_else(|_| "{}".to_string());

        let workspace = Workspace {
            id: id.clone(),
            task_id: input.task_id.clone(),
            run_id: input.run_id,
            workspace_type: input.workspace_type,
            path: input.path.clone(),
            branch_name: input.branch_name,
            base_ref: input.base_ref,
            repo_root: input.repo_root,
            task_scope_path: input.task_scope_path,
            scope_mode: input.scope_mode.unwrap_or(ScopeMode::Repo),
            status: WorkspaceStatus::Creating,
            created_at: now.clone(),
            cleaned_at: None,
            metadata_json,
        };

        self.conn
            .execute(
                "INSERT INTO workspaces (
                    id, task_id, run_id, workspace_type, path, branch_name, base_ref,
                    repo_root, task_scope_path, scope_mode, status, created_at, cleaned_at, metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    workspace.id,
                    workspace.task_id,
                    workspace.run_id,
                    workspace.workspace_type.to_string(),
                    workspace.path,
                    workspace.branch_name,
                    workspace.base_ref,
                    workspace.repo_root,
                    workspace.task_scope_path,
                    workspace.scope_mode.to_string(),
                    workspace.status.to_string(),
                    workspace.created_at,
                    workspace.cleaned_at,
                    workspace.metadata_json,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Workspace created: {} (task: {})", id, input.task_id);
        Ok(workspace)
    }

    /// Get a workspace by ID
    pub fn get(&self, id: &str) -> Result<Option<Workspace>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM workspaces WHERE id = ?1",
            params![id],
            Self::row_to_workspace,
        );

        match result {
            Ok(workspace) => Ok(Some(workspace)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Update workspace status
    pub fn update_status(&self, id: &str, status: WorkspaceStatus) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        let cleaned_at = match status {
            WorkspaceStatus::Cleaned => Some(now),
            _ => None,
        };

        let rows_affected = self
            .conn
            .execute(
                "UPDATE workspaces SET status = ?1, cleaned_at = COALESCE(?2, cleaned_at) WHERE id = ?3",
                params![status.to_string(), cleaned_at, id],
            )
            .map_err(DatabaseError::QueryError)?;

        if rows_affected == 0 {
            debug!("No workspace found to update: {}", id);
        } else {
            debug!("Workspace status updated: {} -> {}", id, status);
        }

        Ok(())
    }

    /// Get active workspaces (Ready or Active status)
    pub fn get_active_workspaces(&self) -> Result<Vec<Workspace>, DatabaseError> {
        self.query_workspaces(
            "SELECT * FROM workspaces WHERE status IN ('READY', 'ACTIVE') ORDER BY created_at DESC",
            params![],
        )
    }

    /// Get workspaces that need cleanup (Cleaned status or old)
    pub fn get_workspaces_for_cleanup(&self) -> Result<Vec<Workspace>, DatabaseError> {
        self.query_workspaces(
            "SELECT * FROM workspaces WHERE status IN ('CLEANING', 'FAILED') ORDER BY created_at ASC",
            params![],
        )
    }

    /// Mark a workspace as cleaned up
    pub fn cleanup_workspace(&self, id: &str) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        self.conn
            .execute(
                "UPDATE workspaces SET status = 'CLEANED', cleaned_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Workspace cleaned up: {}", id);
        Ok(())
    }

    /// List workspaces with filtering options
    pub fn list(&self, options: WorkspaceListOptions) -> Result<Vec<Workspace>, DatabaseError> {
        let limit = options.limit.unwrap_or(100);
        let offset = options.offset.unwrap_or(0);

        let workspaces = if let Some(task_id) = &options.task_id {
            self.query_workspaces(
                "SELECT * FROM workspaces WHERE task_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                params![task_id, limit as i64, offset as i64],
            )?
        } else if let Some(run_id) = &options.run_id {
            self.query_workspaces(
                "SELECT * FROM workspaces WHERE run_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                params![run_id, limit as i64, offset as i64],
            )?
        } else if let Some(statuses) = &options.status {
            let placeholders: Vec<String> = statuses.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT * FROM workspaces WHERE status IN ({}) ORDER BY created_at DESC LIMIT ? OFFSET ?",
                placeholders.join(",")
            );
            let mut params: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
            params.push(limit.to_string());
            params.push(offset.to_string());
            self.query_workspaces_with_params(&sql, &params)?
        } else {
            self.query_workspaces(
                "SELECT * FROM workspaces ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
                params![limit as i64, offset as i64],
            )?
        };

        Ok(workspaces)
    }

    /// Get workspace for a specific task run
    pub fn get_for_run(&self, run_id: &str) -> Result<Option<Workspace>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM workspaces WHERE run_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![run_id],
            Self::row_to_workspace,
        );

        match result {
            Ok(workspace) => Ok(Some(workspace)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Delete a workspace record
    pub fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute("DELETE FROM workspaces WHERE id = ?1", params![id])
            .map_err(DatabaseError::QueryError)?;

        debug!("Workspace deleted: {}", id);
        Ok(())
    }

    /// Helper to query workspaces
    fn query_workspaces<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<Workspace>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Self::row_to_workspace)
            .map_err(DatabaseError::QueryError)?;

        let mut workspaces = Vec::new();
        for row in rows {
            workspaces.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(workspaces)
    }

    /// Helper to query workspaces with string params
    fn query_workspaces_with_params(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<Workspace>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), Self::row_to_workspace)
            .map_err(DatabaseError::QueryError)?;

        let mut workspaces = Vec::new();
        for row in rows {
            workspaces.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(workspaces)
    }

    /// Map a database row to a Workspace
    fn row_to_workspace(row: &Row<'_>) -> rusqlite::Result<Workspace> {
        let workspace_type_str: String = row.get("workspace_type")?;
        let status_str: String = row.get("status")?;
        let scope_mode_str: String = row.get("scope_mode")?;

        Ok(Workspace {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            run_id: row.get("run_id")?,
            workspace_type: workspace_type_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            path: row.get("path")?,
            branch_name: row.get("branch_name")?,
            base_ref: row.get("base_ref")?,
            repo_root: row.get("repo_root")?,
            task_scope_path: row.get("task_scope_path")?,
            scope_mode: scope_mode_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            status: status_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            created_at: row.get("created_at")?,
            cleaned_at: row.get("cleaned_at")?,
            metadata_json: row.get("metadata_json")?,
        })
    }
}
