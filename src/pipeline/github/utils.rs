//! Utility Functions for GitHub Integration
//!
//! Extraction helpers for task metadata: workspace info, execution policy,
//! sync state, task links, and policy validation.

use anyhow::Result;

use super::types::{ExecutionPolicy, GitHubSyncState, GitHubTaskLink, GitHubTaskWorkspace};
use crate::pipeline::engine::PipelineRunResult;
use crate::pipeline::phases::{Phase, Task};

/// Helper to get GitHub config from an integration DTO
pub fn orchestrator_github_config(
    config: Option<crate::config::GitHubIntegration>,
) -> Option<(super::types::GitHubConfig, crate::config::GitHubIntegration)> {
    let integration = config?;
    let repository = integration.repository.clone().into_iter().collect();
    Some((
        super::types::GitHubConfig {
            repositories: repository,
            trigger_labels: vec!["d3vx".to_string()],
            auto_process_labels: vec!["d3vx-auto".to_string()],
            poll_interval_secs: 300,
            webhook_secret: None,
            sync_status: true,
            token_env: integration.token_env.clone(),
            api_base_url: integration.api_base_url.clone(),
        },
        integration,
    ))
}

pub fn extract_github_task_link(
    task: &Task,
    config: &crate::config::GitHubIntegration,
) -> Option<GitHubTaskLink> {
    let metadata = task.metadata.as_object()?;

    if let Some(github) = metadata.get("github").and_then(|v| v.as_object()) {
        let repository = github
            .get("repository")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| config.repository.clone())?;
        let issue_number = github.get("issue_number").and_then(|v| v.as_u64());
        return Some(GitHubTaskLink {
            repository,
            issue_number,
        });
    }

    if let Some(source) = metadata.get("source").and_then(|v| v.as_object()) {
        if let Some(issue) = source.get("github_issue").and_then(|v| v.as_object()) {
            let repository = issue
                .get("repository")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| config.repository.clone())?;
            let issue_number = issue.get("number").and_then(|v| v.as_u64());
            return Some(GitHubTaskLink {
                repository,
                issue_number,
            });
        }

        if let Some(comment) = source.get("github_pr_comment").and_then(|v| v.as_object()) {
            let repository = comment
                .get("repository")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| config.repository.clone())?;
            let issue_number = comment.get("pr_number").and_then(|v| v.as_u64());
            return Some(GitHubTaskLink {
                repository,
                issue_number,
            });
        }
    }

    config.repository.as_ref().map(|repository| GitHubTaskLink {
        repository: repository.clone(),
        issue_number: None,
    })
}

pub fn extract_task_workspace(task: &Task) -> GitHubTaskWorkspace {
    let mut project_path = task.project_root.clone();
    let mut branch = task.branch.clone();
    let mut worktree_path = task.worktree_path.clone();

    if let Some(source) = task
        .metadata
        .as_object()
        .and_then(|map| map.get("source"))
        .and_then(|value| value.as_object())
        .and_then(|source| source.get("vex"))
        .and_then(|value| value.as_object())
    {
        if project_path.is_none() {
            project_path = source
                .get("project_path")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if branch.is_none() {
            branch = source
                .get("branch")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
    }

    if let Some(workspace) = task
        .metadata
        .as_object()
        .and_then(|map| map.get("workspace"))
        .and_then(|value| value.as_object())
    {
        if project_path.is_none() {
            project_path = workspace
                .get("project_path")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if branch.is_none() {
            branch = workspace
                .get("branch_name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if worktree_path.is_none() {
            worktree_path = workspace
                .get("worktree_path")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
    }

    GitHubTaskWorkspace {
        project_path,
        branch,
        worktree_path,
    }
}

pub fn extract_execution_policy(task: &Task) -> ExecutionPolicy {
    let policy = task
        .metadata
        .as_object()
        .and_then(|map| map.get("execution_policy"))
        .and_then(|value| value.as_object());

    ExecutionPolicy {
        review_required: policy
            .and_then(|map| map.get("review_required"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_merge_if_safe: policy
            .and_then(|map| map.get("auto_merge_if_safe"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        docs_required: policy
            .and_then(|map| map.get("docs_required"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

pub fn extract_github_sync_state(task: &Task) -> GitHubSyncState {
    let sync = task
        .metadata
        .as_object()
        .and_then(|map| map.get("github_sync"))
        .and_then(|value| value.as_object());

    GitHubSyncState {
        started_comment_posted: sync
            .and_then(|map| map.get("started_comment_posted_at"))
            .is_some(),
        completed_comment_posted: sync
            .and_then(|map| map.get("completed_comment_posted_at"))
            .is_some(),
        pull_request_url: sync
            .and_then(|map| map.get("pull_request_url"))
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        merged_at: sync
            .and_then(|map| map.get("merged_at"))
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    }
}

pub fn validate_execution_policy_result(
    result: &PipelineRunResult,
    policy: ExecutionPolicy,
) -> Result<()> {
    if !result.success {
        return Ok(());
    }

    if policy.review_required && !result.phase_results.contains_key(&Phase::Review) {
        anyhow::bail!(
            "review was required for task {} but the Review phase did not complete",
            result.task.id
        );
    }

    if policy.docs_required && !result.phase_results.contains_key(&Phase::Docs) {
        anyhow::bail!(
            "docs were required for task {} but the Docs phase did not complete",
            result.task.id
        );
    }

    Ok(())
}
