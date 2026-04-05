//! GitHub Integration Types
//!
//! Type definitions for GitHub webhook events, API responses, configuration,
//! and integration structs used across the github module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// Event Types
// ═══════════════════════════════════════════════════════════════════════════════

/// GitHub webhook event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GitHubEvent {
    /// Issue opened
    IssueOpened {
        number: u64,
        repository: String,
        author: String,
        title: String,
        body: Option<String>,
        labels: Vec<String>,
    },
    /// Issue labeled
    IssueLabeled {
        number: u64,
        repository: String,
        label: String,
        actor: String,
    },
    /// Issue closed
    IssueClosed {
        number: u64,
        repository: String,
        actor: String,
    },
    /// PR review requested
    PRReviewRequested {
        number: u64,
        repository: String,
        author: String,
        title: String,
        requested_reviewer: String,
    },
    /// PR comment
    PRComment {
        number: u64,
        comment_id: u64,
        repository: String,
        author: String,
        body: String,
    },
    /// PR changes requested
    PRChangesRequested {
        number: u64,
        repository: String,
        reviewer: String,
        comment: Option<String>,
    },
    /// CI status changed
    CIStatusChanged {
        repository: String,
        branch: String,
        commit_sha: String,
        status: CIStatus,
        context: String,
        description: Option<String>,
        target_url: Option<String>,
    },
    /// Check run completed
    CheckRunCompleted {
        repository: String,
        branch: String,
        commit_sha: String,
        check_name: String,
        status: CheckStatus,
        conclusion: Option<String>,
        output: Option<CheckOutput>,
    },
}

/// CI status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CIStatus {
    Pending,
    Success,
    Failure,
    Error,
}

/// Check run status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Queued,
    InProgress,
    Completed,
}

/// Check run output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutput {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub text: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for GitHub integration
#[derive(Debug, Clone)]
pub struct GitHubConfig {
    /// Repositories to monitor
    pub repositories: Vec<String>,
    /// Labels that trigger task creation
    pub trigger_labels: Vec<String>,
    /// Auto-process issues with these labels
    pub auto_process_labels: Vec<String>,
    /// Poll interval for checking new issues (seconds)
    pub poll_interval_secs: u64,
    /// Webhook secret for validation
    pub webhook_secret: Option<String>,
    /// Whether to sync status back to GitHub
    pub sync_status: bool,
    /// Environment variable containing the GitHub token
    pub token_env: String,
    /// GitHub API base URL
    pub api_base_url: String,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            repositories: Vec::new(),
            trigger_labels: vec!["d3vx".to_string(), "ai-assist".to_string()],
            auto_process_labels: vec!["d3vx-auto".to_string()],
            poll_interval_secs: 300,
            webhook_secret: None,
            sync_status: true,
            token_env: "GITHUB_TOKEN".to_string(),
            api_base_url: "https://api.github.com".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Response Types
// ═══════════════════════════════════════════════════════════════════════════════

/// GitHub pull request info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub number: u64,
    pub html_url: String,
    pub state: String,
}

/// GitHub API issue response
#[derive(Debug, Deserialize)]
pub(crate) struct GitHubIssueResponse {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) body: Option<String>,
    pub(crate) state: String,
    pub(crate) labels: Vec<GitHubLabel>,
    pub(crate) user: GitHubUser,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) pull_request: Option<serde_json::Value>,
}

/// GitHub label
#[derive(Debug, Deserialize)]
pub(crate) struct GitHubLabel {
    pub(crate) name: String,
}

/// GitHub user
#[derive(Debug, Deserialize)]
pub(crate) struct GitHubUser {
    pub(crate) login: String,
}

/// GitHub API pull request response
#[derive(Debug, Deserialize)]
pub(crate) struct GitHubPullRequestResponse {
    pub(crate) number: u64,
    pub(crate) html_url: String,
    pub(crate) state: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Request Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Request body for creating an issue
#[derive(Debug, Serialize)]
pub(crate) struct CreateIssueRequest<'a> {
    pub(crate) title: &'a str,
    pub(crate) body: &'a str,
    pub(crate) labels: Vec<String>,
}

/// Request body for creating a pull request
#[derive(Debug, Serialize)]
pub(crate) struct CreatePullRequestRequest<'a> {
    pub(crate) title: &'a str,
    pub(crate) head: &'a str,
    pub(crate) base: &'a str,
    pub(crate) body: &'a str,
}

/// Request body for creating a comment
#[derive(Debug, Serialize)]
pub(crate) struct CreateCommentRequest<'a> {
    pub(crate) body: &'a str,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Integration Types
// ═══════════════════════════════════════════════════════════════════════════════

/// GitHub issue representation (returned by API client and poller)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workspace paths extracted from task metadata
#[derive(Debug, Clone)]
pub struct GitHubTaskWorkspace {
    pub project_path: Option<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
}

/// Execution policy controlling task lifecycle requirements
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionPolicy {
    pub review_required: bool,
    pub auto_merge_if_safe: bool,
    pub docs_required: bool,
}

/// Sync state tracking what has been posted back to GitHub
#[derive(Debug, Clone, Default)]
pub struct GitHubSyncState {
    pub started_comment_posted: bool,
    pub completed_comment_posted: bool,
    pub pull_request_url: Option<String>,
    pub merged_at: Option<String>,
}

/// Link between a task and a GitHub issue
#[derive(Debug, Clone)]
pub struct GitHubTaskLink {
    pub repository: String,
    pub issue_number: Option<u64>,
}
