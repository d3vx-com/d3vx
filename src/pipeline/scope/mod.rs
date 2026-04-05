//! Scope Handling for Tasks

pub mod resolver;
#[cfg(test)]
mod tests;
pub mod types;
pub mod workspace;

pub use resolver::{find_nested_repos, find_repo_root, is_nested_repo, TaskScope};
pub use types::{ScopeError, ScopeMode};
pub use workspace::ScopeAwareWorkspace;
