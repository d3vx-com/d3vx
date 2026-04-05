//! GitHub Integration Module
//!
//! Provides GitHub webhook handling, polling for issue/PR ingestion,
//! task synchronization, and git operations for merge/PR automation.
//! All GitHub events are normalized into tasks through the intake layer.

mod api;
mod git_ops;
mod polling;
mod sync;
mod tests;
mod types;
mod utils;
mod webhook;

pub use types::*;

pub use api::GitHubApiClient;
pub use git_ops::{ensure_branch_merge_ready, push_branch, run_git, validate_workspace_for_merge};
pub use polling::{GitHubManager, GitHubPoller};
pub use sync::{
    maybe_merge_task_branch, maybe_raise_pull_request, sync_github_task_finished,
    sync_github_task_started,
};
pub use utils::{
    extract_execution_policy, extract_github_sync_state, extract_github_task_link,
    extract_task_workspace, orchestrator_github_config, validate_execution_policy_result,
};
pub use webhook::GitHubIntegration;
