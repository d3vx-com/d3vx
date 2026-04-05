//! Workspace store for managing task execution environments
//!
//! Workspaces provide isolated environments for task execution,
//! including direct paths, git worktrees, and mirrored directories.

mod store;
#[cfg(test)]
mod tests;
mod types;

pub use store::WorkspaceStore;
pub use types::*;
