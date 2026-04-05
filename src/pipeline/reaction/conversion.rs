//! Conversion from GitHub events to reaction events.

use super::types::ReactionEvent;
use crate::pipeline::github::{CIStatus, GitHubEvent};

impl ReactionEvent {
    /// Convert from GitHub event
    pub fn from_github_event(event: &GitHubEvent, task_id: Option<String>) -> Option<Self> {
        match event {
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
                    return None;
                }
                Some(ReactionEvent::CIFailure {
                    repository: repository.clone(),
                    branch: branch.clone(),
                    commit_sha: commit_sha.clone(),
                    context: context.clone(),
                    description: description.clone().unwrap_or_default(),
                    target_url: target_url.clone(),
                    task_id,
                })
            }
            GitHubEvent::PRComment {
                number,
                repository,
                author,
                body,
                ..
            } => Some(ReactionEvent::ReviewComment {
                pr_number: *number,
                repository: repository.clone(),
                author: author.clone(),
                body: body.clone(),
                changes_requested: false,
                task_id,
            }),
            GitHubEvent::PRChangesRequested {
                number,
                repository,
                reviewer,
                comment,
            } => Some(ReactionEvent::ReviewComment {
                pr_number: *number,
                repository: repository.clone(),
                author: reviewer.clone(),
                body: comment.clone().unwrap_or_default(),
                changes_requested: true,
                task_id,
            }),
            _ => None,
        }
    }
}
