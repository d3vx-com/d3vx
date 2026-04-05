//! Issue Runner
//!
//! Manages a single GitHub issue through its own isolated pipeline:
//! worktree creation → pipeline execution → result collection.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use super::engine::{PipelineEngine, PipelineRunResult};
use super::github::{sync_github_task_finished, sync_github_task_started};
use super::phases::{PhaseContext, Task};
use crate::agent::AgentLoop;

/// State of an issue's pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueRunnerState {
    /// Worktree is being created
    SettingUp,
    /// Pipeline is running
    Running,
    /// Pipeline completed
    Completed,
    /// Pipeline failed
    Failed,
}

/// Result of running a single issue through the pipeline.
#[derive(Debug, Clone)]
pub struct IssueRunResult {
    /// The task that was executed
    pub task: Task,
    /// Pipeline result (if it ran)
    pub pipeline_result: Option<PipelineRunResult>,
    /// Final state
    pub state: IssueRunnerState,
    /// Any error message
    pub error: Option<String>,
}

/// Runs a single GitHub issue through the full pipeline in isolation.
pub struct IssueRunner {
    /// The task being executed
    pub task: Task,
    /// Path to the git worktree for this issue
    pub worktree_path: Option<PathBuf>,
    /// Current state
    pub state: IssueRunnerState,
    /// Result after completion
    pub result: Option<IssueRunResult>,
}

impl IssueRunner {
    /// Create a new issue runner.
    pub fn new(task: Task) -> Self {
        Self {
            task,
            worktree_path: None,
            state: IssueRunnerState::SettingUp,
            result: None,
        }
    }

    /// Set up the worktree and initialize the pipeline context.
    pub async fn setup(&mut self, repo_root: &str, worktree_base: &str) -> anyhow::Result<()> {
        // Create a unique branch name
        let branch_name = format!("d3vx-issue-{}", self.task.id.to_lowercase());
        let worktree_path = format!("{}/{}", worktree_base, branch_name);

        // Create the git worktree if it doesn't exist
        let worktree_exists = tokio::fs::metadata(&worktree_path).await.is_ok();
        if !worktree_exists {
            let output = tokio::process::Command::new("git")
                .args([
                    "worktree",
                    "add",
                    &worktree_path,
                    "--no-checkout",
                    "-b",
                    &branch_name,
                ])
                .current_dir(repo_root)
                .output()
                .await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Worktree might already exist from a previous run
                if !stderr.contains("already exists") {
                    return Err(anyhow::anyhow!("Failed to create worktree: {}", stderr));
                }
            }
            info!(
                worktree = %worktree_path, branch = %branch_name,
                "Created worktree for issue runner"
            );
        }

        self.worktree_path = Some(PathBuf::from(worktree_path));
        self.state = IssueRunnerState::Running;
        Ok(())
    }

    /// Run the issue through the full pipeline.
    pub async fn run(
        &mut self,
        engine: Arc<PipelineEngine>,
        agent: Option<Arc<crate::agent::AgentLoop>>,
    ) -> PipelineRunResult {
        let context = self.build_context();

        match engine.run(self.task.clone(), context).await {
            Ok(result) => {
                self.state = if result.success {
                    IssueRunnerState::Completed
                } else {
                    IssueRunnerState::Failed
                };
                self.result = Some(IssueRunResult {
                    task: result.task.clone(),
                    pipeline_result: Some(result.clone()),
                    state: self.state.clone(),
                    error: result.error.clone(),
                });
                result
            }
            Err(e) => {
                self.state = IssueRunnerState::Failed;
                self.result = Some(IssueRunResult {
                    task: self.task.clone(),
                    pipeline_result: None,
                    state: self.state.clone(),
                    error: Some(e.to_string()),
                });
                PipelineRunResult::failure(self.task.clone(), e.to_string())
            }
        }
    }

    /// Sync the result back to GitHub (comment on issue, raise PR).
    pub async fn sync_to_github(
        &self,
        github_config: Option<crate::config::GitHubIntegration>,
        integration_cfg: Option<&crate::config::GitHubIntegration>,
        agent: Option<Arc<crate::agent::AgentLoop>>,
    ) -> anyhow::Result<()> {
        let Some(result) = &self.result else {
            return Ok(());
        };

        // Sync task started
        let _ = sync_github_task_started(github_config.clone(), &self.task).await;

        // Sync task finished (posts comment, raises PR if configured)
        if let Some(pipeline_result) = &result.pipeline_result {
            let _ = sync_github_task_finished(github_config, pipeline_result, agent.clone()).await;
        }

        Ok(())
    }

    fn build_context(&self) -> PhaseContext {
        let repo_root = self
            .worktree_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_string();

        let worktree = self
            .worktree_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_string();

        PhaseContext::new(self.task.clone(), &repo_root, &worktree)
    }
}
