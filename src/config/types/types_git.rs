//! Git integration configuration

use serde::{Deserialize, Serialize};

/// Git integration configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GitConfig {
    /// Automatically commit changes
    #[serde(default = "default_true")]
    pub auto_commit: bool,
    /// Automatically push after commit
    #[serde(default)]
    pub auto_push: bool,
    /// Directory for git worktrees
    #[serde(default = "default_worktree_dir")]
    pub worktree_dir: String,
    /// Prefix for commit messages
    #[serde(default = "default_commit_prefix")]
    pub commit_prefix: String,
    /// Use AI to generate commit messages
    #[serde(default = "default_true")]
    pub ai_commit_messages: bool,
    /// Max tokens for AI-generated commit messages
    #[serde(default = "default_commit_message_max_tokens")]
    pub commit_message_max_tokens: u32,
    /// Main branch name
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
    /// Sign commits with GPG
    #[serde(default)]
    pub sign_commits: bool,
    /// Pre-commit hook configurations
    #[serde(default)]
    pub pre_commit_hooks: PreCommitConfig,
}

/// Pre-commit hook configurations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PreCommitConfig {
    #[serde(default = "default_true")]
    pub format: bool,
    #[serde(default = "default_true")]
    pub clippy: bool,
    #[serde(default = "default_true")]
    pub test: bool,
    #[serde(default = "default_true")]
    pub security: bool,
    #[serde(default = "default_true")]
    pub skip_if_wip: bool,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u32,
}

impl Default for PreCommitConfig {
    fn default() -> Self {
        Self {
            format: true,
            clippy: true,
            test: true,
            security: true,
            skip_if_wip: true,
            timeout_seconds: 60,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_worktree_dir() -> String {
    ".d3vx-worktrees".to_string()
}

fn default_commit_prefix() -> String {
    "feat".to_string()
}

fn default_commit_message_max_tokens() -> u32 {
    100
}

fn default_main_branch() -> String {
    "main".to_string()
}

fn default_timeout_seconds() -> u32 {
    60
}
