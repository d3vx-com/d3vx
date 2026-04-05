//! Scope Resolver
//!
//! Repository detection, nested repo finding, task scope, and workspace provisioning.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::info;

use super::types::{ScopeError, ScopeMode};

/// Find the root of a git repository by walking up the directory tree
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start;

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Some(current.to_path_buf());
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

/// Check if a path is inside a nested git repository
/// (i.e., there's a .git somewhere between start and the outer repo_root)
pub fn is_nested_repo(path: &Path, outer_repo_root: &Path) -> bool {
    let mut current = path;

    while current != outer_repo_root {
        if let Some(parent) = current.parent() {
            // Check if this directory has its own .git
            if parent.join(".git").exists() && parent != outer_repo_root {
                return true;
            }
            current = parent;
        } else {
            break;
        }
    }

    false
}

/// Find all nested repositories within a path
pub fn find_nested_repos(root: &Path) -> Vec<PathBuf> {
    let mut nested = Vec::new();
    let mut visited = HashSet::new();

    fn scan_dir(dir: &Path, nested: &mut Vec<PathBuf>, visited: &mut HashSet<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let git_dir = path.join(".git");
                    if git_dir.exists() {
                        if visited.insert(path.clone()) {
                            nested.push(path.clone());
                            // Don't recurse into nested repos
                            continue;
                        }
                    }
                    // Skip common non-project directories
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if ![
                        "node_modules",
                        "target",
                        ".git",
                        "vendor",
                        "__pycache__",
                        "build",
                    ]
                    .contains(&name)
                    {
                        scan_dir(&path, nested, visited);
                    }
                }
            }
        }
    }

    scan_dir(root, &mut nested, &mut visited);
    nested
}

/// Task scope metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskScope {
    /// Root of the project/workspace
    pub project_root: PathBuf,
    /// Root of the git repository (for git operations, worktrees)
    pub repo_root: PathBuf,
    /// Path the task is allowed to modify (may be same as repo_root or a subdirectory)
    pub task_scope_path: PathBuf,
    /// How the scope relates to the repository
    pub scope_mode: ScopeMode,
    /// Whether task is allowed to expand scope
    pub allow_scope_expansion: bool,
}

impl TaskScope {
    /// Create a repo-wide scope
    pub fn repo_wide(project_root: PathBuf) -> Self {
        let repo_root = find_repo_root(&project_root).unwrap_or_else(|| project_root.clone());
        Self {
            project_root: project_root.clone(),
            repo_root: repo_root.clone(),
            task_scope_path: repo_root,
            scope_mode: ScopeMode::Repo,
            allow_scope_expansion: false,
        }
    }

    /// Create a subdirectory-scoped task
    pub fn subdir(project_root: PathBuf, subdir: PathBuf) -> Self {
        let repo_root = find_repo_root(&project_root).unwrap_or_else(|| project_root.clone());
        let task_scope_path = if subdir.is_absolute() {
            subdir
        } else {
            project_root.join(&subdir)
        };
        Self {
            project_root,
            repo_root,
            task_scope_path,
            scope_mode: ScopeMode::Subdir,
            allow_scope_expansion: true,
        }
    }

    /// Create a nested repo scope
    pub fn nested_repo(nested_root: PathBuf, _parent_root: Option<PathBuf>) -> Self {
        Self {
            project_root: nested_root.clone(),
            repo_root: nested_root.clone(),
            task_scope_path: nested_root.clone(),
            scope_mode: ScopeMode::NestedRepo,
            allow_scope_expansion: false,
        }
    }

    /// Create a multi-repo scope (for parent tasks)
    pub fn multi_repo(project_root: PathBuf, _repos: Vec<PathBuf>) -> Self {
        Self {
            project_root,
            repo_root: PathBuf::new(), // No single repo root
            task_scope_path: PathBuf::new(),
            scope_mode: ScopeMode::MultiRepo,
            allow_scope_expansion: false,
        }
    }

    /// Detect scope from a starting path
    pub fn detect(from_path: &Path) -> Self {
        let canonical = match from_path.canonicalize() {
            Ok(p) => p,
            Err(_) => from_path.to_path_buf(),
        };

        // Find git root
        let repo_root = find_repo_root(&canonical).unwrap_or_else(|| canonical.clone());

        // Check if we're at repo root or in a subdirectory
        if canonical == repo_root {
            // At repo root
            Self::repo_wide(canonical)
        } else if is_nested_repo(&canonical, &repo_root) {
            // We're in a nested repo
            let nested_root = find_repo_root(&canonical).unwrap_or(canonical.clone());
            Self::nested_repo(nested_root, Some(repo_root))
        } else {
            // We're in a subdirectory of the repo
            Self::subdir(repo_root.clone(), canonical)
        }
    }

    /// Check if a path is within the allowed scope
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        match self.scope_mode {
            ScopeMode::Repo | ScopeMode::NestedRepo => canonical.starts_with(&self.task_scope_path),
            ScopeMode::Subdir => canonical.starts_with(&self.task_scope_path),
            ScopeMode::MultiRepo => {
                // For multi-repo, check against project root
                canonical.starts_with(&self.project_root)
            }
        }
    }

    /// Get relative path from scope root
    pub fn relative_path(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.task_scope_path)
            .ok()
            .map(|p| p.to_path_buf())
    }

    /// Expand scope to include a new path
    pub fn expand_scope(&mut self, new_path: &Path) -> Result<(), ScopeError> {
        if !self.allow_scope_expansion {
            return Err(ScopeError::ExpansionNotAllowed);
        }

        let canonical = new_path
            .canonicalize()
            .map_err(|e| ScopeError::InvalidPath(format!("{}: {}", new_path.display(), e)))?;

        // Make sure the new path is still within the repo
        if !canonical.starts_with(&self.repo_root) {
            return Err(ScopeError::PathOutsideRepo(canonical));
        }

        // Update scope if the new path is outside current scope
        if !canonical.starts_with(&self.task_scope_path) {
            self.task_scope_path = self.repo_root.clone();
            self.scope_mode = ScopeMode::Repo;
            info!(
                "Expanded task scope to repo root: {}",
                self.repo_root.display()
            );
        }

        Ok(())
    }

    /// Get the worktree branch name for this scope
    pub fn suggested_branch_name(&self, task_id: &str) -> String {
        let suffix = match self.scope_mode {
            ScopeMode::Subdir => {
                // Include subdirectory name in branch
                let dir_name = self
                    .task_scope_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("scope");
                format!("-{}", dir_name)
            }
            ScopeMode::NestedRepo => {
                let repo_name = self
                    .repo_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("nested");
                format!("-{}", repo_name)
            }
            _ => String::new(),
        };

        format!("d3vx/{}{}", task_id, suffix)
    }
}
