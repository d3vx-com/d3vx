//! GitHub Task Synchronization
//!
//! Syncs task lifecycle events (started, finished) back to GitHub,
//! including posting comments, raising PRs, and auto-merging branches.

use anyhow::Result;
use std::sync::Arc;

use super::api::GitHubApiClient;
use super::git_ops;
use super::types::GitHubTaskWorkspace;
use super::utils;
use crate::agent::AgentLoop;
use crate::pipeline::engine::PipelineRunResult;
use crate::pipeline::phases::Task;

pub async fn sync_github_task_started(
    config: Option<crate::config::GitHubIntegration>,
    task: &Task,
) -> Result<Option<serde_json::Value>> {
    let Some((github_config, integration)) = utils::orchestrator_github_config(config) else {
        return Ok(None);
    };
    if !integration.auto_raise_prs && integration.repository.is_none() {
        return Ok(None);
    }

    let Some(link) = utils::extract_github_task_link(task, &integration) else {
        return Ok(None);
    };
    let Some(issue_number) = link.issue_number else {
        return Ok(None);
    };

    let sync_state = utils::extract_github_sync_state(task);
    if sync_state.started_comment_posted {
        return Ok(None);
    };

    let client = GitHubApiClient::from_config(&github_config)?;
    let body = format!(
        "d3vx started autonomous work on `{}`.\n\nTask ID: `{}`\nPhase: `{}`\nStatus: `{}`",
        task.title, task.id, task.phase, task.status
    );
    client
        .create_issue_comment(&link.repository, issue_number, &body)
        .await?;
    Ok(Some(serde_json::json!({
        "github_sync": {
            "started_comment_posted_at": chrono::Utc::now().to_rfc3339()
        }
    })))
}

pub async fn sync_github_task_finished(
    config: Option<crate::config::GitHubIntegration>,
    result: &PipelineRunResult,
    agent: Option<Arc<AgentLoop>>,
) -> Result<Option<serde_json::Value>> {
    let Some((github_config, integration)) = utils::orchestrator_github_config(config) else {
        return Ok(None);
    };

    let Some(link) = utils::extract_github_task_link(&result.task, &integration) else {
        return Ok(None);
    };

    let client = GitHubApiClient::from_config(&github_config)?;
    let workspace = utils::extract_task_workspace(&result.task);
    let policy = utils::extract_execution_policy(&result.task);
    let sync_state = utils::extract_github_sync_state(&result.task);

    let mut completion_lines = vec![
        if result.success {
            format!("d3vx completed `{}` successfully.", result.task.title)
        } else {
            format!("d3vx failed while working on `{}`.", result.task.title)
        },
        format!("Task ID: `{}`", result.task.id),
        format!("Final phase: `{}`", result.task.phase),
    ];

    if let Some(branch) = &workspace.branch {
        completion_lines.push(format!("Branch: `{}`", branch));
    }
    if let Some(error) = &result.error {
        completion_lines.push(format!("Error: {}", error));
    }
    if policy.review_required {
        completion_lines.push("Policy: review required before finalization.".to_string());
    }
    if policy.docs_required {
        completion_lines.push("Policy: docs are required for this task.".to_string());
    }

    let policy_validation = utils::validate_execution_policy_result(result, policy.clone());
    if let Err(error) = &policy_validation {
        completion_lines.push(format!("Policy validation failed: {}", error));
    }

    let mut pr_url = None;
    let mut merge_summary = None;
    if result.success && policy_validation.is_ok() && policy.auto_merge_if_safe {
        if sync_state.merged_at.is_none() {
            match maybe_merge_task_branch(&integration, &result.task, &workspace, agent.clone())
                .await
            {
                Ok(summary) => {
                    merge_summary = Some(summary);
                }
                Err(error) => {
                    completion_lines.push(format!("Auto-merge failed: {}", error));
                }
            }
        } else {
            merge_summary = Some(format!(
                "Branch was already merged at {}.",
                sync_state.merged_at.as_deref().unwrap_or("unknown time")
            ));
        }
    } else if result.success && policy_validation.is_ok() && integration.auto_raise_prs {
        if let Some(existing_url) = sync_state.pull_request_url.clone() {
            pr_url = Some(existing_url);
        } else {
            match maybe_raise_pull_request(
                &client,
                &integration,
                &link.repository,
                &result.task,
                &workspace,
                link.issue_number,
                agent.clone(),
            )
            .await
            {
                Ok(url) => {
                    pr_url = url;
                }
                Err(error) => {
                    completion_lines.push(format!("PR automation failed: {}", error));
                }
            }
        }
    }

    if let Some(summary) = &merge_summary {
        completion_lines.push(summary.clone());
    }
    if let Some(url) = &pr_url {
        completion_lines.push(format!("Pull request: {}", url));
    }

    if let Some(issue_number) = link
        .issue_number
        .filter(|_| !sync_state.completed_comment_posted)
    {
        client
            .create_issue_comment(&link.repository, issue_number, &completion_lines.join("\n"))
            .await?;
    }

    let mut github_sync = serde_json::Map::new();
    if !sync_state.completed_comment_posted {
        github_sync.insert(
            "completed_comment_posted_at".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
    }
    if let Some(url) = pr_url {
        github_sync.insert(
            "pull_request_url".to_string(),
            serde_json::Value::String(url),
        );
    }
    if merge_summary.is_some() && sync_state.merged_at.is_none() && policy.auto_merge_if_safe {
        github_sync.insert(
            "merged_at".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
    }

    if github_sync.is_empty() {
        Ok(None)
    } else {
        Ok(Some(serde_json::json!({ "github_sync": github_sync })))
    }
}

pub async fn maybe_merge_task_branch(
    integration: &crate::config::GitHubIntegration,
    task: &Task,
    workspace: &GitHubTaskWorkspace,
    agent: Option<Arc<AgentLoop>>,
) -> Result<String> {
    let Some(branch) = workspace.branch.as_deref() else {
        return Err(anyhow::anyhow!("task {} has no branch metadata", task.id));
    };
    let merge_path = workspace
        .worktree_path
        .as_deref()
        .or(workspace.project_path.as_deref())
        .ok_or_else(|| anyhow::anyhow!("task {} has no workspace path", task.id))?;
    let repo_path = workspace.project_path.as_deref().unwrap_or(merge_path);

    git_ops::ensure_branch_merge_ready(merge_path, &integration.default_branch, agent).await?;
    git_ops::run_git(repo_path, &["fetch", "origin", &integration.default_branch]).await?;
    git_ops::run_git(repo_path, &["checkout", &integration.default_branch]).await?;
    git_ops::run_git(
        repo_path,
        &["pull", "--ff-only", "origin", &integration.default_branch],
    )
    .await?;
    git_ops::validate_workspace_for_merge(merge_path).await?;
    git_ops::run_git(repo_path, &["merge", "--no-ff", branch]).await?;
    git_ops::run_git(repo_path, &["push", "origin", &integration.default_branch]).await?;

    Ok(format!(
        "Merged branch `{}` into `{}` and pushed the updated base branch.",
        branch, integration.default_branch
    ))
}

pub async fn maybe_raise_pull_request(
    client: &GitHubApiClient,
    integration: &crate::config::GitHubIntegration,
    repository: &str,
    task: &Task,
    workspace: &GitHubTaskWorkspace,
    issue_number: Option<u64>,
    agent: Option<Arc<AgentLoop>>,
) -> Result<Option<String>> {
    let Some(branch) = workspace.branch.as_deref() else {
        return Ok(None);
    };

    if let Some(existing) = client
        .find_open_pull_request(repository, branch, &integration.default_branch)
        .await?
    {
        return Ok(Some(existing.html_url));
    }

    let merge_path = workspace
        .worktree_path
        .as_deref()
        .or(workspace.project_path.as_deref());
    if let Some(merge_path) = merge_path {
        git_ops::ensure_branch_merge_ready(merge_path, &integration.default_branch, agent).await?;
        git_ops::validate_workspace_for_merge(merge_path).await?;
        git_ops::push_branch(merge_path, branch).await?;
    }

    let mut body = vec![
        format!("Automated PR created by d3vx for task `{}`.", task.id),
        format!("Task: {}", task.title),
    ];

    if let Some(issue_number) = issue_number {
        body.push(format!("Refs: #{}", issue_number));
    }

    let pr = client
        .create_pull_request(
            repository,
            &format!("d3vx: {}", task.title),
            branch,
            &integration.default_branch,
            &body.join("\n\n"),
        )
        .await?;

    Ok(Some(pr.html_url))
}
