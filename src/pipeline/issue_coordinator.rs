//! Issue Coordinator
//!
//! Dispatches multiple GitHub issues to independent pipelines running
//! in parallel worktrees with resource throttling.
//!
//! ## Flow
//!
//! ```text
//! Pick issues → Create worktrees → Run pipelines in parallel → Collect results → Report
//! ```
//!
//! ## Throttling
//!
//! Uses a semaphore to limit concurrent pipelines (default: 3).
//! Each issue gets its own worktree, its own pipeline engine clone,
//! and its own async task.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use super::engine::PipelineEngine;
use super::issue_picker::IssuePicker;
use super::issue_runner::IssueRunner;
use super::issue_sync::types::ExternalIssue;
use super::phases::{Task, TaskStatus};

/// Configuration for parallel issue resolution.
#[derive(Debug, Clone)]
pub struct IssueCoordinationConfig {
    /// Max concurrent issue pipelines
    pub max_concurrent_issues: usize,
    /// Base path for worktrees
    pub worktree_base: String,
    /// GitHub repo to target
    pub repository: String,
}

impl Default for IssueCoordinationConfig {
    fn default() -> Self {
        Self {
            max_concurrent_issues: 3,
            worktree_base: ".d3vx-worktrees".to_string(),
            repository: String::new(),
        }
    }
}

/// Result of coordinating multiple issues.
#[derive(Debug, Clone)]
pub struct IssueCoordinationResult {
    /// Total issues processed
    pub total: usize,
    /// Successful completions
    pub successful: usize,
    /// Failed completions
    pub failed: usize,
    /// Skipped (below pick threshold)
    pub skipped: usize,
    /// Per-issue results
    pub issue_results: Vec<SingleIssueResult>,
}

/// Result for a single issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleIssueResult {
    pub issue_number: Option<u64>,
    pub issue_title: String,
    pub task_id: String,
    pub success: bool,
    pub error: Option<String>,
    pub state: String,
}

/// Coordinates parallel resolution of multiple GitHub issues.
pub struct IssueCoordinator {
    config: IssueCoordinationConfig,
    engine: Arc<PipelineEngine>,
    picker: Arc<IssuePicker>,
}

impl IssueCoordinator {
    pub fn new(
        config: IssueCoordinationConfig,
        engine: Arc<PipelineEngine>,
        picker: Arc<IssuePicker>,
    ) -> Self {
        Self {
            config,
            engine,
            picker,
        }
    }

    /// Pick the best issues from the pool and run them in parallel.
    ///
    /// Returns results ordered by priority score (highest first).
    pub async fn resolve_issues(
        &self,
        issues: &[ExternalIssue],
        agent: Option<Arc<crate::agent::AgentLoop>>,
    ) -> IssueCoordinationResult {
        // Use the picker to score and rank issues
        let decision = self.picker.pick(issues);

        let mut results = IssueCoordinationResult {
            total: issues.len(),
            successful: 0,
            failed: 0,
            skipped: issues.len().saturating_sub(decision.all_scores.len()),
            issue_results: Vec::new(),
        };

        // Only process issues that passed the pick threshold
        let actionable: Vec<_> = decision
            .all_scores
            .iter()
            .filter(|(_, score)| score.is_pickable())
            .cloned()
            .collect();

        if actionable.is_empty() {
            warn!("No actionable issues found after picking");
            return results;
        }

        info!(
            count = actionable.len(),
            "Starting parallel issue resolution"
        );

        // Create runners for each issue
        let runners: Vec<_> = actionable
            .into_iter()
            .map(|(issue, _score)| self.build_runner(&issue).ok())
            .filter_map(|r| r)
            .collect();

        // Extract owned values before spawning tasks (lifetime fix)
        let max_concurrent = self.config.max_concurrent_issues;
        let worktree_base = self.config.worktree_base.clone();
        let engine = self.engine.clone();
        let agent_ref = agent.clone();

        // Run them in parallel with throttling
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        let mut handles = Vec::new();
        for runner in runners {
            let sem = semaphore.clone();
            let engine = engine.clone();
            let agent = agent_ref.clone();
            let worktree_base = worktree_base.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await;

                // Setup worktree
                let repo_root = std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .unwrap_or_default();

                let mut runner = runner;
                if let Err(e) = runner.setup(&repo_root, &worktree_base).await {
                    error!("Runner setup failed: {}", e);
                    return (runner, None, Some(e.to_string()));
                }

                // Run pipeline
                let result = runner.run(engine, agent).await;
                let err = result.error.clone();

                (runner, Some(result), err)
            });

            handles.push(handle);
        }

        // Collect results
        for handle in handles {
            match handle.await {
                Ok((runner, pipeline_result, error)) => {
                    let success = pipeline_result.as_ref().map_or(false, |r| r.success);
                    let state = match &runner.state {
                        super::issue_runner::IssueRunnerState::Completed => "completed",
                        super::issue_runner::IssueRunnerState::Failed => "failed",
                        _ => "unknown",
                    };

                    if success {
                        results.successful += 1;
                    } else {
                        results.failed += 1;
                    }

                    let issue_number = runner
                        .task
                        .metadata
                        .get("github_issue_number")
                        .and_then(|v| v.as_u64());

                    results.issue_results.push(SingleIssueResult {
                        issue_number,
                        issue_title: runner.task.title.clone(),
                        task_id: runner.task.id.clone(),
                        success,
                        error,
                        state: state.to_string(),
                    });
                }
                Err(e) => {
                    results.failed += 1;
                    results.issue_results.push(SingleIssueResult {
                        issue_number: None,
                        issue_title: "unknown".to_string(),
                        task_id: "unknown".to_string(),
                        success: false,
                        error: Some(format!("Task join error: {e}")),
                        state: "failed".to_string(),
                    });
                }
            }
        }

        info!(
            successful = results.successful,
            failed = results.failed,
            skipped = results.skipped,
            "Parallel issue resolution completed"
        );

        results
    }

    fn build_runner(&self, issue: &ExternalIssue) -> anyhow::Result<IssueRunner> {
        let instruction = format!(
            "GitHub Issue #{}\n\n## {}\n\n{}",
            issue.number.unwrap_or(0),
            issue.title,
            issue.body.as_deref().unwrap_or("No description provided")
        );

        let task_id = format!("ISSUE-{}", issue.id);
        let mut task = Task::new(&task_id, &issue.title, &instruction);
        task.status = TaskStatus::Queued;

        // Store issue number in metadata for later retrieval
        if let Some(num) = issue.number {
            task.metadata = serde_json::json!({
                "github_issue_number": num,
                "github_url": issue.url
            });
        }

        Ok(IssueRunner::new(task))
    }
}
