//! Transition probes: inspect workspace state to infer the current session phase.

use std::path::Path;

use async_trait::async_trait;
use tracing::{debug, warn};

use super::types::SessionPhase;

// ============================================================================
// TRAIT
// ============================================================================

/// Probes a workspace to detect which session phase the worktree is in.
#[async_trait]
pub trait TransitionProbe: Send + Sync {
    /// Inspect `workspace_path` and return the detected phase, or `None` if
    /// the probe cannot determine the phase.
    async fn probe_workspace(&self, workspace_path: &Path) -> Option<SessionPhase>;
}

// ============================================================================
// GIT PROBE
// ============================================================================

/// Inspects git and GitHub CLI state to infer the session phase.
pub struct GitProbe;

impl GitProbe {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransitionProbe for GitProbe {
    async fn probe_workspace(&self, workspace_path: &Path) -> Option<SessionPhase> {
        let pr_state = detect_pr_state(workspace_path).await;
        if pr_state.is_some() {
            debug!(path = %workspace_path.display(), "GitProbe detected PR state");
            return pr_state;
        }

        let ci_state = detect_ci_state(workspace_path).await;
        if ci_state.is_some() {
            debug!(path = %workspace_path.display(), "GitProbe detected CI state");
            return ci_state;
        }

        let review_state = detect_review_state(workspace_path).await;
        if review_state.is_some() {
            debug!(path = %workspace_path.display(), "GitProbe detected review state");
            return review_state;
        }

        // If there are uncommitted changes, assume agent is still working.
        if has_uncommitted_changes(workspace_path).await {
            debug!(path = %workspace_path.display(), "GitProbe detected working state");
            return Some(SessionPhase::Working);
        }

        None
    }
}

impl Default for GitProbe {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// COMPOSITE PROBE
// ============================================================================

/// Combines multiple probes and returns the most-specific (non-Working) phase.
pub struct CompositeProbe {
    probes: Vec<Box<dyn TransitionProbe>>,
}

impl CompositeProbe {
    pub fn new(probes: Vec<Box<dyn TransitionProbe>>) -> Self {
        Self { probes }
    }
}

#[async_trait]
impl TransitionProbe for CompositeProbe {
    async fn probe_workspace(&self, workspace_path: &Path) -> Option<SessionPhase> {
        let mut best: Option<SessionPhase> = None;

        for probe in &self.probes {
            if let Some(phase) = probe.probe_workspace(workspace_path).await {
                // Prefer more specific phases over generic Working.
                if best.is_none() || phase != SessionPhase::Working {
                    best = Some(phase);
                }
            }
        }

        best
    }
}

// ============================================================================
// AGENT STATUS HELPER
// ============================================================================

/// Map a process exit status to a session phase.
pub fn probe_agent_status(is_running: bool, exit_code: Option<i32>) -> SessionPhase {
    if is_running {
        SessionPhase::Working
    } else {
        match exit_code {
            Some(0) => SessionPhase::Done,
            Some(_) => SessionPhase::Crashed,
            None => SessionPhase::Orphaned,
        }
    }
}

// ============================================================================
// INTERNAL DETECTION HELPERS
// ============================================================================

async fn detect_pr_state(workspace_path: &Path) -> Option<SessionPhase> {
    let output = run_gh(workspace_path, &["pr", "status", "--json", "state"]).await?;
    if output.contains("\"OPEN\"") {
        return Some(SessionPhase::PrOpen);
    }
    if output.contains("\"MERGED\"") {
        return Some(SessionPhase::Merged);
    }
    if output.contains("\"CLOSED\"") {
        return Some(SessionPhase::Cancelled);
    }
    None
}

async fn detect_ci_state(workspace_path: &Path) -> Option<SessionPhase> {
    let output = run_gh(workspace_path, &["pr", "checks"]).await?;
    if output.contains("passing") || output.contains("passed") || output.contains("success") {
        return Some(SessionPhase::CiPassed);
    }
    if output.contains("failing") || output.contains("failed") || output.contains("failure") {
        return Some(SessionPhase::CiFailed);
    }
    if output.contains("pending") || output.contains("running") {
        return Some(SessionPhase::CiRunning);
    }
    None
}

async fn detect_review_state(workspace_path: &Path) -> Option<SessionPhase> {
    let output = run_gh(workspace_path, &["pr", "view", "--json", "reviewDecision"]).await?;
    if output.contains("\"APPROVED\"") {
        return Some(SessionPhase::ApprovedForMerge);
    }
    if output.contains("\"CHANGES_REQUESTED\"") {
        return Some(SessionPhase::ChangesRequested);
    }
    if output.contains("\"REVIEW_REQUIRED\"") {
        return Some(SessionPhase::ReviewPending);
    }
    None
}

async fn has_uncommitted_changes(workspace_path: &Path) -> bool {
    run_git(workspace_path, &["status", "--porcelain"])
        .await
        .map(|out| !out.trim().is_empty())
        .unwrap_or(false)
}

async fn run_git(workspace_path: &Path, args: &[&str]) -> Option<String> {
    tokio::process::Command::new("git")
        .args(args)
        .current_dir(workspace_path)
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

async fn run_gh(workspace_path: &Path, args: &[&str]) -> Option<String> {
    let result = tokio::process::Command::new("gh")
        .args(args)
        .current_dir(workspace_path)
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(output) => {
            warn!(
                "gh {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            None
        }
        Err(e) => {
            warn!("gh {} execution error: {}", args.join(" "), e);
            None
        }
    }
}
