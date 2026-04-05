//! Scope-Aware Workspace Provisioning

use std::path::{Path, PathBuf};

use super::resolver::TaskScope;
use super::types::{ScopeError, ScopeMode};

/// Scope-aware workspace provisioning
#[derive(Debug, Clone)]
pub struct ScopeAwareWorkspace {
    /// The task scope
    pub scope: TaskScope,
    /// Workspace path (may be worktree or shadow copy)
    pub workspace_path: PathBuf,
    /// Whether this is a worktree
    pub is_worktree: bool,
    /// Branch name if worktree
    pub branch: Option<String>,
}

impl ScopeAwareWorkspace {
    /// Plan workspace provisioning based on scope
    pub fn plan(scope: TaskScope, workspace_base: &Path, task_id: &str) -> Self {
        let branch = scope.suggested_branch_name(task_id);
        let workspace_path = workspace_base.join(format!("d3vx-{}", task_id));

        // For subdirs, we may want to create a partial workspace
        // For now, we create full worktrees but scope the task's access
        Self {
            scope,
            workspace_path,
            is_worktree: true,
            branch: Some(branch),
        }
    }

    /// Get the actual path the task should operate in
    pub fn task_working_directory(&self) -> PathBuf {
        if self.scope.scope_mode == ScopeMode::Subdir {
            // Map the scope path to the workspace
            let relative = self
                .scope
                .task_scope_path
                .strip_prefix(&self.scope.repo_root)
                .unwrap_or_else(|_| Path::new(""));
            self.workspace_path.join(relative)
        } else {
            self.workspace_path.clone()
        }
    }

    /// Validate that a path is within scope for this workspace
    pub fn validate_path(&self, path: &Path) -> Result<PathBuf, ScopeError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| ScopeError::InvalidPath(format!("{}: {}", path.display(), e)))?;

        if !self.scope.is_path_allowed(&canonical) {
            return Err(ScopeError::PathOutsideScope(canonical));
        }

        Ok(canonical)
    }
}
