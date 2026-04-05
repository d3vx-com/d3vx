//! Webhook Event Processing
//!
//! Handles incoming GitHub webhook events, deduplicates them, and converts
//! them into task intake inputs for normalization.

use anyhow::Result;
use tracing::{debug, info};

use super::types::{CIStatus, CheckStatus, GitHubConfig, GitHubEvent};
use crate::pipeline::intake::{TaskIntake, TaskIntakeInput};
use crate::pipeline::phases::Priority;

/// GitHub integration handler
pub struct GitHubIntegration {
    config: GitHubConfig,
    intake: TaskIntake,
    processed_events: std::collections::HashSet<String>,
}

impl GitHubIntegration {
    pub fn new(config: GitHubConfig) -> Self {
        Self {
            config,
            intake: TaskIntake::new("GH"),
            processed_events: std::collections::HashSet::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(GitHubConfig::default())
    }

    /// Process a GitHub webhook event
    pub fn process_webhook(
        &mut self,
        event: GitHubEvent,
    ) -> Result<Option<crate::pipeline::phases::Task>> {
        let event_id = self.event_id(&event);
        if self.processed_events.contains(&event_id) {
            debug!("Skipping already processed event: {}", event_id);
            return Ok(None);
        }
        self.processed_events.insert(event_id.clone());

        let input = match self.event_to_intake(&event)? {
            Some(input) => input,
            None => return Ok(None),
        };

        let task = self.intake.normalize_to_task(input)?;
        info!("Created task {} from GitHub event: {}", task.id, event_id);
        Ok(Some(task))
    }

    /// Convert GitHub event to intake input
    pub(crate) fn event_to_intake(&self, event: &GitHubEvent) -> Result<Option<TaskIntakeInput>> {
        match event {
            GitHubEvent::IssueOpened {
                number,
                repository,
                author,
                title,
                body,
                labels,
            } => {
                let has_trigger = labels.iter().any(|l| {
                    self.config.trigger_labels.contains(l)
                        || self.config.auto_process_labels.contains(l)
                });
                if !has_trigger {
                    debug!("Issue #{} has no trigger label, skipping", number);
                    return Ok(None);
                }
                let is_auto = labels
                    .iter()
                    .any(|l| self.config.auto_process_labels.contains(l));
                let input = TaskIntakeInput::from_github_issue(
                    *number,
                    repository.clone(),
                    author.clone(),
                    title.clone(),
                    body.clone().unwrap_or_default(),
                )
                .with_tags(vec!["github".to_string(), "issue".to_string()]);
                let input = if is_auto {
                    input.with_metadata("auto_start".to_string(), serde_json::json!(true))
                } else {
                    input
                };
                Ok(Some(input))
            }

            GitHubEvent::IssueLabeled {
                number,
                repository,
                label,
                actor,
            } => {
                if !self.config.trigger_labels.contains(label)
                    && !self.config.auto_process_labels.contains(label)
                {
                    return Ok(None);
                }
                let input = TaskIntakeInput::from_github_issue(
                    *number,
                    repository.clone(),
                    actor.clone(),
                    format!("Issue #{} labeled with {}", number, label),
                    format!(
                        "Issue #{} was labeled with '{}' by {}, triggering task creation.",
                        number, label, actor
                    ),
                );
                Ok(Some(input))
            }

            GitHubEvent::IssueClosed { .. } => {
                debug!("Issue closed event - no task created");
                Ok(None)
            }

            GitHubEvent::PRReviewRequested {
                number,
                repository,
                author,
                title,
                requested_reviewer: _,
            } => {
                let input = TaskIntakeInput::from_pr_comment(
                    *number,
                    0,
                    repository.clone(),
                    author.clone(),
                    format!("Review requested for PR: {}", title),
                )
                .with_priority(Priority::High);
                Ok(Some(input))
            }

            GitHubEvent::PRComment {
                number,
                comment_id,
                repository,
                author,
                body,
            } => {
                let body_lower = body.to_lowercase();
                if !body_lower.contains("@d3vx") && !body_lower.contains("/d3vx") {
                    debug!("PR comment doesn't mention d3vx, skipping");
                    return Ok(None);
                }
                let input = TaskIntakeInput::from_pr_comment(
                    *number,
                    *comment_id,
                    repository.clone(),
                    author.clone(),
                    body.clone(),
                );
                Ok(Some(input))
            }

            GitHubEvent::PRChangesRequested {
                number,
                repository,
                reviewer,
                comment,
            } => {
                let input = TaskIntakeInput::from_pr_comment(
                    *number,
                    0,
                    repository.clone(),
                    reviewer.clone(),
                    format!(
                        "Changes requested: {}",
                        comment.as_deref().unwrap_or("No details")
                    ),
                )
                .with_priority(Priority::High);
                Ok(Some(input))
            }

            GitHubEvent::CIStatusChanged {
                repository,
                branch,
                commit_sha,
                status,
                context,
                description,
                target_url,
            } => {
                if *status != CIStatus::Failure && *status != CIStatus::Error {
                    return Ok(None);
                }
                let error_details = format!(
                    "CI failure in {} on branch {}\nContext: {}\nDescription: {}\nURL: {}",
                    repository,
                    branch,
                    context,
                    description.as_deref().unwrap_or("No description"),
                    target_url.as_deref().unwrap_or("N/A")
                );
                let input = TaskIntakeInput::from_ci_failure(
                    context.clone(),
                    branch.clone(),
                    commit_sha.clone(),
                    error_details,
                );
                Ok(Some(input))
            }

            GitHubEvent::CheckRunCompleted {
                repository,
                branch,
                commit_sha,
                check_name,
                status,
                conclusion,
                output,
            } => {
                if *status != CheckStatus::Completed {
                    return Ok(None);
                }
                let is_failure = conclusion
                    .as_ref()
                    .map(|c| c == "failure" || c == "timed_out" || c == "cancelled")
                    .unwrap_or(false);
                if !is_failure {
                    return Ok(None);
                }
                let error_details = format!(
                    "Check '{}' failed in {} on branch {}\n{}",
                    check_name,
                    repository,
                    branch,
                    output
                        .as_ref()
                        .and_then(|o| o.summary.clone())
                        .unwrap_or_default()
                );
                let input = TaskIntakeInput::from_ci_failure(
                    check_name.clone(),
                    branch.clone(),
                    commit_sha.clone(),
                    error_details,
                );
                Ok(Some(input))
            }
        }
    }

    /// Generate event ID for deduplication
    pub(crate) fn event_id(&self, event: &GitHubEvent) -> String {
        match event {
            GitHubEvent::IssueOpened {
                number, repository, ..
            } => format!("issue-{}-{}-opened", repository, number),
            GitHubEvent::IssueLabeled {
                number,
                repository,
                label,
                ..
            } => format!("issue-{}-{}-labeled-{}", repository, number, label),
            GitHubEvent::IssueClosed {
                number, repository, ..
            } => format!("issue-{}-{}-closed", repository, number),
            GitHubEvent::PRReviewRequested {
                number, repository, ..
            } => format!("pr-{}-{}-review-requested", repository, number),
            GitHubEvent::PRComment {
                number,
                comment_id,
                repository,
                ..
            } => format!("pr-{}-{}-comment-{}", repository, number, comment_id),
            GitHubEvent::PRChangesRequested {
                number, repository, ..
            } => format!("pr-{}-{}-changes-requested", repository, number),
            GitHubEvent::CIStatusChanged {
                commit_sha,
                context,
                ..
            } => format!("ci-{}-{}", commit_sha, context),
            GitHubEvent::CheckRunCompleted {
                commit_sha,
                check_name,
                ..
            } => format!("check-{}-{}", commit_sha, check_name),
        }
    }

    pub fn intake(&self) -> &TaskIntake {
        &self.intake
    }

    pub fn config(&self) -> &GitHubConfig {
        &self.config
    }

    /// Add a trigger label (pub(crate) for test support)
    #[allow(dead_code)]
    pub fn add_trigger_label(&mut self, label: &str) {
        self.config.trigger_labels.push(label.to_string());
    }
}
