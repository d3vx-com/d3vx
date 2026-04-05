//! Workspace types and data structures
//!
//! Defines the type enums and structs used by the workspace store.

use serde::{Deserialize, Serialize};

/// Type of workspace isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceType {
    /// Direct execution in the project directory
    Direct,
    /// Git worktree for isolated branch work
    Worktree,
    /// Mirrored copy of the repository
    Mirror,
}

impl WorkspaceType {
    /// Get all workspace types
    pub fn all() -> &'static [WorkspaceType] {
        &[
            WorkspaceType::Direct,
            WorkspaceType::Worktree,
            WorkspaceType::Mirror,
        ]
    }
}

impl std::fmt::Display for WorkspaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceType::Direct => write!(f, "direct"),
            WorkspaceType::Worktree => write!(f, "worktree"),
            WorkspaceType::Mirror => write!(f, "mirror"),
        }
    }
}

impl std::str::FromStr for WorkspaceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DIRECT" => Ok(WorkspaceType::Direct),
            "WORKTREE" => Ok(WorkspaceType::Worktree),
            "MIRROR" => Ok(WorkspaceType::Mirror),
            _ => Err(format!("Invalid workspace type: {}", s)),
        }
    }
}

/// Status of a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceStatus {
    /// Workspace is being created
    Creating,
    /// Workspace is ready for use
    Ready,
    /// Workspace is currently in use
    Active,
    /// Workspace is being cleaned up
    Cleaning,
    /// Workspace has been cleaned up
    Cleaned,
    /// Workspace cleanup failed
    Failed,
}

impl std::fmt::Display for WorkspaceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceStatus::Creating => write!(f, "CREATING"),
            WorkspaceStatus::Ready => write!(f, "READY"),
            WorkspaceStatus::Active => write!(f, "ACTIVE"),
            WorkspaceStatus::Cleaning => write!(f, "CLEANING"),
            WorkspaceStatus::Cleaned => write!(f, "CLEANED"),
            WorkspaceStatus::Failed => write!(f, "FAILED"),
        }
    }
}

impl std::str::FromStr for WorkspaceStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "CREATING" => Ok(WorkspaceStatus::Creating),
            "READY" => Ok(WorkspaceStatus::Ready),
            "ACTIVE" => Ok(WorkspaceStatus::Active),
            "CLEANING" => Ok(WorkspaceStatus::Cleaning),
            "CLEANED" => Ok(WorkspaceStatus::Cleaned),
            "FAILED" => Ok(WorkspaceStatus::Failed),
            _ => Err(format!("Invalid workspace status: {}", s)),
        }
    }
}

/// Scope mode for task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeMode {
    /// Entire repository
    Repo,
    /// Subdirectory of the repo
    Subdir,
    /// Nested repository (git submodule or nested .git)
    NestedRepo,
    /// Multiple repositories
    MultiRepo,
}

impl ScopeMode {
    /// Get all scope modes
    pub fn all() -> &'static [ScopeMode] {
        &[
            ScopeMode::Repo,
            ScopeMode::Subdir,
            ScopeMode::NestedRepo,
            ScopeMode::MultiRepo,
        ]
    }
}

impl std::fmt::Display for ScopeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeMode::Repo => write!(f, "REPO"),
            ScopeMode::Subdir => write!(f, "SUBDIR"),
            ScopeMode::NestedRepo => write!(f, "NESTED_REPO"),
            ScopeMode::MultiRepo => write!(f, "MULTI_REPO"),
        }
    }
}

impl std::str::FromStr for ScopeMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "REPO" => Ok(ScopeMode::Repo),
            "SUBDIR" => Ok(ScopeMode::Subdir),
            "NESTED_REPO" => Ok(ScopeMode::NestedRepo),
            "MULTI_REPO" => Ok(ScopeMode::MultiRepo),
            _ => Err(format!("Invalid scope mode: {}", s)),
        }
    }
}

/// A workspace for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique workspace identifier
    pub id: String,
    /// Associated task ID
    pub task_id: String,
    /// Associated run ID
    pub run_id: Option<String>,
    /// Type of workspace
    pub workspace_type: WorkspaceType,
    /// Path to the workspace directory
    pub path: String,
    /// Git branch name (for worktree type)
    pub branch_name: Option<String>,
    /// Base git reference (branch, commit, tag)
    pub base_ref: Option<String>,
    /// Root of the repository
    pub repo_root: Option<String>,
    /// Path within repo for task scope
    pub task_scope_path: Option<String>,
    /// Scope mode for execution
    pub scope_mode: ScopeMode,
    /// Current status
    pub status: WorkspaceStatus,
    /// Creation timestamp
    pub created_at: String,
    /// Cleanup timestamp
    pub cleaned_at: Option<String>,
    /// Additional metadata (JSON)
    pub metadata_json: String,
}

/// Input for creating a new workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWorkspace {
    /// Optional custom ID
    pub id: Option<String>,
    /// Associated task ID
    pub task_id: String,
    /// Associated run ID
    pub run_id: Option<String>,
    /// Workspace type
    pub workspace_type: WorkspaceType,
    /// Workspace path
    pub path: String,
    /// Git branch name
    pub branch_name: Option<String>,
    /// Base reference
    pub base_ref: Option<String>,
    /// Repository root
    pub repo_root: Option<String>,
    /// Task scope path
    pub task_scope_path: Option<String>,
    /// Scope mode
    pub scope_mode: Option<ScopeMode>,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Options for listing workspaces
#[derive(Debug, Clone, Default)]
pub struct WorkspaceListOptions {
    /// Filter by task ID
    pub task_id: Option<String>,
    /// Filter by run ID
    pub run_id: Option<String>,
    /// Filter by status
    pub status: Option<Vec<WorkspaceStatus>>,
    /// Maximum results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}
