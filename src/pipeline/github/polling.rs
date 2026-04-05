//! Issue Polling and Task Management
//!
//! Contains the GitHub poller for periodically checking new issues,
//! and the GitHubManager that integrates with the orchestrator's task factory.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{info, warn};

use super::api::GitHubApiClient;
use super::types::{GitHubConfig, GitHubIssue};
use crate::pipeline::intake::TaskIntakeInput;
use crate::pipeline::phases::Task;
use crate::pipeline::task_factory::TaskFactory;

// ═══════════════════════════════════════════════════════════════════════════════
// Issue Poller
// ═══════════════════════════════════════════════════════════════════════════════

/// GitHub poller for checking new issues
pub struct GitHubPoller {
    /// Configuration
    config: GitHubConfig,
    /// GitHub API client
    client: Option<GitHubApiClient>,
    /// Last check time per repository
    last_check: std::collections::HashMap<String, DateTime<Utc>>,
}

impl GitHubPoller {
    /// Create a new poller
    pub fn new(config: GitHubConfig) -> Self {
        let client = match GitHubApiClient::from_config(&config) {
            Ok(client) => Some(client),
            Err(e) => {
                warn!("GitHub client unavailable: {}", e);
                None
            }
        };
        Self {
            config,
            client,
            last_check: std::collections::HashMap::new(),
        }
    }

    pub async fn poll(&mut self) -> Result<Vec<GitHubIssue>> {
        let Some(client) = &self.client else {
            return Ok(Vec::new());
        };

        let mut issues = Vec::new();
        let labels: Vec<String> = self
            .config
            .trigger_labels
            .iter()
            .chain(self.config.auto_process_labels.iter())
            .cloned()
            .collect();

        for repo in &self.config.repositories {
            let since = self.last_check.get(repo).cloned();
            match client.fetch_open_issues(repo, since, &labels).await {
                Ok(mut repo_issues) => {
                    issues.append(&mut repo_issues);
                }
                Err(e) => {
                    warn!("Failed to poll {}: {}", repo, e);
                }
            }
            self.last_check.insert(repo.clone(), Utc::now());
        }

        Ok(issues)
    }

    /// Convert polled issues to intake inputs
    pub fn issues_to_intake(&self, issues: Vec<GitHubIssue>) -> Vec<TaskIntakeInput> {
        issues
            .into_iter()
            .filter(|issue| {
                issue.labels.iter().any(|l| {
                    self.config.trigger_labels.contains(l)
                        || self.config.auto_process_labels.contains(l)
                })
            })
            .map(|issue| {
                TaskIntakeInput::from_github_issue(
                    issue.number,
                    issue.repository,
                    issue.author,
                    issue.title,
                    issue.body.unwrap_or_default(),
                )
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GitHub Manager (Orchestrator Integration)
// ═══════════════════════════════════════════════════════════════════════════════

/// Manager for GitHub operations, integrating with the orchestrator's task factory.
pub struct GitHubManager {
    task_factory: Arc<TaskFactory>,
}

impl GitHubManager {
    pub fn new(task_factory: Arc<TaskFactory>) -> Self {
        Self { task_factory }
    }

    /// Process a GitHub webhook event and create a task if applicable
    pub async fn process_event(&self, event: super::types::GitHubEvent) -> Result<Option<Task>> {
        use super::types::{CIStatus, GitHubEvent};

        match event {
            GitHubEvent::IssueOpened {
                number,
                repository,
                author,
                title,
                body,
                ..
            } => {
                let task = self
                    .task_factory
                    .create_from_github_issue(
                        number,
                        &repository,
                        &author,
                        &title,
                        &body.unwrap_or_default(),
                    )
                    .await?;
                info!(
                    "Created task {} from GitHub issue #{} in {}",
                    task.id, number, repository
                );
                Ok(Some(task))
            }
            GitHubEvent::IssueLabeled {
                number,
                repository,
                actor,
                label,
            } => {
                let task = self
                    .task_factory
                    .create_from_github_issue(
                        number,
                        &repository,
                        &actor,
                        &format!("Issue #{} labeled with {}", number, label),
                        &format!(
                            "Issue #{} was labeled with '{}' by {}",
                            number, label, actor
                        ),
                    )
                    .await?;
                Ok(Some(task))
            }
            GitHubEvent::PRComment {
                number,
                comment_id,
                repository,
                author,
                body,
            } => {
                let task = self
                    .task_factory
                    .create_from_pr_comment(number, comment_id, &repository, &author, &body)
                    .await?;
                Ok(Some(task))
            }
            GitHubEvent::PRChangesRequested {
                number,
                repository,
                reviewer,
                comment,
            } => {
                let task = self
                    .task_factory
                    .create_from_pr_comment(
                        number,
                        0,
                        &repository,
                        &reviewer,
                        &format!("Changes requested: {}", comment.unwrap_or_default()),
                    )
                    .await?;
                Ok(Some(task))
            }
            GitHubEvent::CIStatusChanged {
                repository,
                branch,
                commit_sha,
                status,
                context,
                description,
                ..
            } => {
                if status == CIStatus::Failure || status == CIStatus::Error {
                    let task = self
                        .task_factory
                        .create_from_ci_failure(
                            &format!("{}-{}", repository, commit_sha),
                            &branch,
                            &commit_sha,
                            &format!(
                                "CI failure in {}: {}",
                                context,
                                description.unwrap_or_default()
                            ),
                        )
                        .await?;
                    Ok(Some(task))
                } else {
                    Ok(None)
                }
            }
            _ => {
                tracing::debug!("GitHub event type not handled for task creation");
                Ok(None)
            }
        }
    }

    /// Start a background GitHub poller that creates tasks from new issues
    pub async fn start_poller(&self, config: GitHubConfig) -> Result<()> {
        let poll_interval_secs = config.poll_interval_secs;
        let mut poller = GitHubPoller::new(config);
        let task_factory = self.task_factory.clone();

        tokio::spawn(async move {
            loop {
                match poller.poll().await {
                    Ok(issues) => {
                        for issue in issues {
                            match task_factory
                                .create_from_github_issue(
                                    issue.number,
                                    &issue.repository,
                                    &issue.author,
                                    &issue.title,
                                    &issue.body.unwrap_or_default(),
                                )
                                .await
                            {
                                Ok(task) => {
                                    info!(
                                        "Created task {} from polled GitHub issue #{}",
                                        task.id, issue.number
                                    );
                                }
                                Err(e) => {
                                    warn!("Failed to create task from polled GitHub issue: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("GitHub poller error: {}", e);
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await;
            }
        });

        Ok(())
    }
}
