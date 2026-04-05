pub mod auto_review;
pub mod checks;
pub mod engine;
pub mod prompt;
pub mod registry;
pub mod types;

// Re-export new engine and types for convenience.
pub use auto_review::{
    check_post_edit_quality, AutoReviewConfig, QualityGateResult, ReviewFinding, Severity,
};
pub use engine::HookEngine;
pub use types::*;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    Execution(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Clone)]
pub struct HookContext {
    pub changed_files: Vec<PathBuf>,
    pub commit_message: String,
    pub worktree_path: PathBuf,
}

#[derive(Debug)]
pub enum HookResult {
    Pass,
    Fail(String),
    Skip(String),
}

pub trait PreCommitHook: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError>;
}
