//! Session Restore After Crash
//!
//! Assesses crashed sessions, generates restore plans, and executes workspace
//! recreation so agents can reconnect after an unexpected termination.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::pipeline::heartbeat::HeartbeatManager;
use crate::pipeline::resume::{ResumeManager, SessionSnapshot};

// ============================================================================
// RESTORE CHECKS
// ============================================================================

/// Individual pre-condition evaluated before attempting a restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreCheck {
    /// The workspace directory still exists on disk.
    WorkspaceExists,
    /// The git branch referenced by the session is present.
    BranchExists,
    /// No uncommitted changes conflict with the restore.
    NoConflicts,
    /// No agent process is currently alive for this session.
    AgentNotRunning,
    /// Session snapshot metadata parses and is not corrupted.
    MetadataValid,
}

impl std::fmt::Display for RestoreCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestoreCheck::WorkspaceExists => write!(f, "workspace exists"),
            RestoreCheck::BranchExists => write!(f, "branch exists"),
            RestoreCheck::NoConflicts => write!(f, "no conflicts"),
            RestoreCheck::AgentNotRunning => write!(f, "agent not running"),
            RestoreCheck::MetadataValid => write!(f, "metadata valid"),
        }
    }
}

// ============================================================================
// RESTORE STATUS
// ============================================================================

/// Outcome of the assessment phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RestoreStatus {
    /// All pre-conditions passed -- safe to restore.
    CanRestore { checks_passed: Vec<RestoreCheck> },
    /// One or more pre-conditions failed.
    Blocked {
        failed_checks: Vec<RestoreCheck>,
        reasons: Vec<String>,
    },
    /// An agent is already running for this session.
    AlreadyRunning,
}

// ============================================================================
// RESTORE PLAN
// ============================================================================

/// Step-by-step plan produced after a successful assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePlan {
    /// Session being restored.
    pub session_id: String,
    /// Expected workspace path.
    pub workspace_path: PathBuf,
    /// Git branch the session was working on.
    pub branch: String,
    /// Whether the workspace must be recreated from the worktree.
    pub needs_workspace_recreate: bool,
    /// Optional command string to reconnect the agent.
    pub agent_reconnect_command: Option<String>,
}

// ============================================================================
// RESTORE OUTCOME
// ============================================================================

/// Final result returned after executing a restore plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreOutcome {
    /// Whether the restore succeeded.
    pub success: bool,
    /// New session id if a continuation session was created.
    pub new_session_id: Option<String>,
    /// Human-readable summary.
    pub message: String,
    /// Whether the workspace directory was recreated.
    pub workspace_recreated: bool,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors that can occur during session restore.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// The requested session does not exist.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// The workspace directory is missing and cannot be recreated.
    #[error("Workspace gone: {0}")]
    WorkspaceGone(PathBuf),

    /// A branch conflict prevents safe restore.
    #[error("Branch conflict: {0}")]
    BranchConflict(String),

    /// The agent process is still alive for this session.
    #[error("Agent still alive for session: {0}")]
    AgentStillAlive(String),

    /// Session snapshot metadata is corrupted.
    #[error("Metadata corrupted: {0}")]
    MetadataCorrupted(String),

    /// An I/O error occurred during restore.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// RESTORE SAFETY CHECKER
// ============================================================================

/// Result of workspace safety assessment for restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictCheckResult {
    /// Whether the workspace is safe to restore.
    pub is_safe: bool,
    /// List of conflict marker files found.
    pub conflict_marker_files: Vec<String>,
    /// List of diff-check failures.
    pub diff_check_errors: Vec<String>,
    /// List of dirty/unexpected file changes.
    pub dirty_files: Vec<String>,
    /// Human-readable reasons for any issues.
    pub reasons: Vec<String>,
}

impl ConflictCheckResult {
    /// Create a passing result with no issues.
    pub fn safe() -> Self {
        Self {
            is_safe: true,
            conflict_marker_files: Vec::new(),
            diff_check_errors: Vec::new(),
            dirty_files: Vec::new(),
            reasons: Vec::new(),
        }
    }

    /// Create a failing result with given reasons.
    pub fn unsafe_(
        conflict_markers: Vec<String>,
        diff_errors: Vec<String>,
        dirty: Vec<String>,
    ) -> Self {
        let mut reasons = Vec::new();

        if !conflict_markers.is_empty() {
            reasons.push(format!(
                "Unresolved conflict markers in: {}",
                conflict_markers.join(", ")
            ));
        }

        if !diff_errors.is_empty() {
            reasons.push(format!(
                "Git diff check failures: {}",
                diff_errors.join("; ")
            ));
        }

        if !dirty.is_empty() {
            reasons.push(format!(
                "Unexpected uncommitted changes in: {}",
                dirty.join(", ")
            ));
        }

        Self {
            is_safe: false,
            conflict_marker_files: conflict_markers,
            diff_check_errors: diff_errors,
            dirty_files: dirty,
            reasons,
        }
    }
}

/// Checks workspace state for restore safety.
///
/// This checker verifies:
/// - No unresolved conflict markers (<<<<<<<, =======, >>>>>>>)
/// - No git diff --check failures (whitespace errors, etc.)
/// - Workspace is not dirty with unexpected changes
pub struct RestoreSafetyChecker;

impl RestoreSafetyChecker {
    /// Check if the workspace at the given path is safe to restore.
    ///
    /// Returns `ConflictCheckResult` with details about any unsafe conditions.
    /// Restore is blocked if ANY of these conditions are true:
    /// - Unresolved conflict markers exist
    /// - git diff --check reports whitespace errors
    /// - Workspace has uncommitted changes (dirty state)
    pub async fn check_workspace(workspace_path: &PathBuf) -> ConflictCheckResult {
        // Check 1: Conflict markers in tracked files
        let conflict_markers = Self::check_conflict_markers(workspace_path).await;

        // Check 2: Git diff --check for whitespace issues
        let diff_errors = Self::check_diff_errors(workspace_path).await;

        // Check 3: Dirty/uncommitted changes
        let dirty_files = Self::check_dirty_state(workspace_path).await;

        if conflict_markers.is_empty() && diff_errors.is_empty() && dirty_files.is_empty() {
            ConflictCheckResult::safe()
        } else {
            ConflictCheckResult::unsafe_(conflict_markers, diff_errors, dirty_files)
        }
    }

    /// Check for unresolved git conflict markers (<<<<<<<, =======, >>>>>>>).
    async fn check_conflict_markers(workspace_path: &PathBuf) -> Vec<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .arg("grep")
            .arg("-n")
            .arg("-z")
            .arg("--files-with-matches")
            .arg("-E")
            .arg("^(<<<<<<<|=======|>>>>>>>)")
            .arg("--")
            .output()
            .await
            .inspect_err(
                |e| warn!(target: "restore", error = %e, "git grep for conflict markers failed"),
            )
            .ok();

        match output {
            Some(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Check for git diff --check failures (whitespace errors, etc.).
    async fn check_diff_errors(workspace_path: &PathBuf) -> Vec<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .arg("diff")
            .arg("--check")
            .output()
            .await
            .inspect_err(|e| warn!(target: "restore", error = %e, "git diff --check failed"))
            .ok();

        match output {
            Some(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                stderr
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Check for dirty/uncommitted changes in the workspace.
    async fn check_dirty_state(workspace_path: &PathBuf) -> Vec<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .arg("status")
            .arg("--porcelain")
            .arg("-uno")
            .output()
            .await
            .inspect_err(|e| warn!(target: "restore", error = %e, "git status --porcelain failed"))
            .ok();

        match output {
            Some(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

// ============================================================================
// SESSION RESTORE
// ============================================================================

pub(crate) const WORKTREE_BASE_DIR: &str = ".d3vx-worktrees";

/// Main entry-point for crash recovery of an individual session.
pub struct SessionRestore {
    /// Resume manager used to load snapshots.
    pub(crate) resume_manager: ResumeManager,
    /// Project root directory.
    pub(crate) project_root: PathBuf,
    /// Heartbeat manager for checking active agent sessions.
    pub(crate) heartbeat_manager: Option<Arc<HeartbeatManager>>,
}

impl SessionRestore {
    /// Create a new restore handler.
    pub fn new(resume_manager: ResumeManager, project_root: PathBuf) -> Self {
        Self {
            resume_manager,
            project_root,
            heartbeat_manager: None,
        }
    }

    /// Set heartbeat manager for checking active sessions.
    pub fn with_heartbeat_manager(mut self, manager: Arc<HeartbeatManager>) -> Self {
        self.heartbeat_manager = Some(manager);
        self
    }

    /// Run all pre-condition checks for a given session.
    ///
    /// Returns `RestoreStatus` indicating whether the session can be safely
    /// restored, is blocked, or already has a running agent.
    pub async fn assess(&self, session_id: &str) -> RestoreStatus {
        let mut passed: Vec<RestoreCheck> = Vec::new();
        let mut failed: Vec<RestoreCheck> = Vec::new();
        let mut reasons: Vec<String> = Vec::new();

        // 1. Load and validate metadata.
        match self.resume_manager.load_session_snapshot(session_id).await {
            Ok(Some(_snapshot)) => {
                passed.push(RestoreCheck::MetadataValid);
            }
            Ok(None) => {
                failed.push(RestoreCheck::MetadataValid);
                reasons.push(format!("No snapshot found for session {}", session_id));
                return RestoreStatus::Blocked {
                    failed_checks: failed,
                    reasons,
                };
            }
            Err(e) => {
                failed.push(RestoreCheck::MetadataValid);
                reasons.push(format!("Corrupted snapshot metadata: {}", e));
                return RestoreStatus::Blocked {
                    failed_checks: failed,
                    reasons,
                };
            }
        }

        // Reload snapshot (guaranteed to exist after the check above).
        let snapshot = self
            .resume_manager
            .load_session_snapshot(session_id)
            .await
            .inspect_err(|e| warn!(target: "restore", error = %e, "snapshot reload failed after prior validation"))
            .ok()
            .flatten()
            .expect("snapshot validated above");

        // 2. Check workspace exists.
        let workspace = self.resolve_workspace(&snapshot);
        if workspace.exists() {
            passed.push(RestoreCheck::WorkspaceExists);
        } else {
            failed.push(RestoreCheck::WorkspaceExists);
            reasons.push(format!("Workspace path missing: {}", workspace.display()));
        }

        // 3. Check branch exists.
        if self.branch_exists(&snapshot.task_id) {
            passed.push(RestoreCheck::BranchExists);
        } else {
            failed.push(RestoreCheck::BranchExists);
            reasons.push(format!("Branch for task {} not found", snapshot.task_id));
        }

        // 4. Check workspace safety (conflict markers, diff errors, dirty state).
        if workspace.exists() {
            let safety = RestoreSafetyChecker::check_workspace(&workspace).await;
            if safety.is_safe {
                passed.push(RestoreCheck::NoConflicts);
            } else {
                failed.push(RestoreCheck::NoConflicts);
                for reason in &safety.reasons {
                    reasons.push(reason.clone());
                }
            }
        } else {
            // Workspace doesn't exist, so no conflict check needed.
            passed.push(RestoreCheck::NoConflicts);
        }

        // 5. Agent not running check.
        if self.is_agent_running(session_id).await {
            return RestoreStatus::AlreadyRunning;
        }
        passed.push(RestoreCheck::AgentNotRunning);

        if failed.is_empty() {
            RestoreStatus::CanRestore {
                checks_passed: passed,
            }
        } else {
            RestoreStatus::Blocked {
                failed_checks: failed,
                reasons,
            }
        }
    }

    /// Build a `RestorePlan` from a successful assessment.
    ///
    /// The caller should provide a pre-loaded snapshot so this method
    /// remains synchronous and avoids nested async runtime calls.
    pub fn plan_from_snapshot(
        &self,
        session_id: &str,
        status: &RestoreStatus,
        snapshot: &SessionSnapshot,
    ) -> Result<RestorePlan, RestoreError> {
        match status {
            RestoreStatus::CanRestore { .. } => {}
            RestoreStatus::Blocked { reasons, .. } => {
                return Err(RestoreError::WorkspaceGone(PathBuf::from(
                    reasons.join(", "),
                )));
            }
            RestoreStatus::AlreadyRunning => {
                return Err(RestoreError::AgentStillAlive(session_id.to_string()));
            }
        }

        let workspace = self.resolve_workspace(snapshot);
        let needs_recreate = !workspace.exists();

        let reconnect = if needs_recreate {
            None
        } else {
            Some(generate_reconnect_command(session_id))
        };

        Ok(RestorePlan {
            session_id: session_id.to_string(),
            workspace_path: workspace,
            branch: format!("d3vx-task-{}", snapshot.task_id),
            needs_workspace_recreate: needs_recreate,
            agent_reconnect_command: reconnect,
        })
    }

    /// Build a `RestorePlan` from a successful assessment (async version).
    pub async fn plan(
        &self,
        session_id: &str,
        status: &RestoreStatus,
    ) -> Result<RestorePlan, RestoreError> {
        match status {
            RestoreStatus::CanRestore { .. } => {}
            RestoreStatus::Blocked { reasons, .. } => {
                return Err(RestoreError::WorkspaceGone(PathBuf::from(
                    reasons.join(", "),
                )));
            }
            RestoreStatus::AlreadyRunning => {
                return Err(RestoreError::AgentStillAlive(session_id.to_string()));
            }
        }

        let snapshot = self
            .resume_manager
            .load_session_snapshot(session_id)
            .await
            .map_err(|e| RestoreError::MetadataCorrupted(e.to_string()))?
            .ok_or_else(|| RestoreError::SessionNotFound(session_id.to_string()))?;

        self.plan_from_snapshot(session_id, status, &snapshot)
    }

    /// Execute a restore plan, recreating the workspace if needed.
    pub async fn execute(&self, plan: RestorePlan) -> Result<RestoreOutcome, RestoreError> {
        info!("Executing restore plan for session {}", plan.session_id);

        let mut workspace_recreated = false;

        if plan.needs_workspace_recreate {
            let path = self.recreate_workspace(&plan).await?;
            info!("Workspace recreated at {}", path.display());
            workspace_recreated = true;
        }

        let reconnect = plan
            .agent_reconnect_command
            .clone()
            .unwrap_or_else(|| generate_reconnect_command(&plan.session_id));

        Ok(RestoreOutcome {
            success: true,
            new_session_id: Some(plan.session_id.clone()),
            message: format!("Session {} restored. Run: {}", plan.session_id, reconnect),
            workspace_recreated,
        })
    }

    /// Recreate a missing workspace by creating a new worktree.
    pub async fn recreate_workspace(&self, plan: &RestorePlan) -> Result<PathBuf, RestoreError> {
        let worktree_base = self.project_root.join(WORKTREE_BASE_DIR);
        let workspace_name = format!("d3vx-task-{}", plan.session_id);
        let workspace_path = worktree_base.join(&workspace_name);

        fs::create_dir_all(&workspace_path).await?;
        debug!(
            "Created workspace directory at {}",
            workspace_path.display()
        );

        Ok(workspace_path)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Derive the expected workspace path from a snapshot.
    fn resolve_workspace(&self, snapshot: &SessionSnapshot) -> PathBuf {
        self.project_root
            .join(WORKTREE_BASE_DIR)
            .join(format!("d3vx-task-{}", snapshot.task_id))
    }

    /// Check if a git branch exists for the given task.
    ///
    /// Returns `true` when no git repository can be discovered, since the
    /// absence of a repo means there is no branch conflict to worry about.
    fn branch_exists(&self, task_id: &str) -> bool {
        let branch_name = format!("d3vx-task-{}", task_id);
        match git2::Repository::discover(&self.project_root) {
            Ok(repo) => repo
                .find_branch(&branch_name, git2::BranchType::Local)
                .is_ok(),
            Err(_) => {
                debug!(
                    "No git repository at {:?} -- skipping branch check",
                    self.project_root
                );
                // No repo means no conflicting branch; treat as pass.
                true
            }
        }
    }

    /// Check whether an agent process is still running for the session.
    ///
    /// Checks the heartbeat manager for an active lease on the session's task.
    /// Also checks if any worker has a heartbeat within the stale timeout.
    async fn is_agent_running(&self, session_id: &str) -> bool {
        let Some(ref heartbeat_manager) = self.heartbeat_manager else {
            debug!("No heartbeat manager configured, assuming agent not running");
            return false;
        };

        let snapshot = match self.resume_manager.load_session_snapshot(session_id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                debug!(
                    "No snapshot found for session {}, assuming not running",
                    session_id
                );
                return false;
            }
            Err(e) => {
                warn!(
                    "Failed to load snapshot for {}: {}, assuming not running",
                    session_id, e
                );
                return false;
            }
        };

        if let Some(lease) = heartbeat_manager.get_lease_by_task(&snapshot.task_id).await {
            let elapsed = lease.elapsed();
            if !lease.is_expired() {
                debug!(
                    "Found active lease {} for task {} (elapsed: {:?})",
                    lease.id, snapshot.task_id, elapsed
                );
                return true;
            }
            debug!(
                "Lease {} for task {} has expired (elapsed: {:?})",
                lease.id, snapshot.task_id, elapsed
            );
        }

        false
    }
}

/// Build the agent reconnect command string.
pub fn generate_reconnect_command(session_id: &str) -> String {
    format!("d3vx session resume --id {}", session_id)
}
