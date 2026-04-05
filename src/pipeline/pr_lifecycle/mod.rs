//! PR Lifecycle Automation
//!
//! Manages the PR lifecycle: creation, CI monitoring, review tracking, and merging.
//! Uses the `gh` CLI for all GitHub interactions, parsing JSON output.

pub mod manager;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export all public types at module level for backward compatibility.
pub use manager::PrLifecycleManager;
pub use types::{CheckConclusion, CiStatus, PrError, PrMetadata, PrState, ReviewInfo, ReviewState};
