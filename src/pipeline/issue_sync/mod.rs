//! Issue Tracker Sync
//!
//! Bidirectional sync between GitHub/Linear issues and d3vx tasks.
//! GitHub operations use the `gh` CLI; Linear is a future stub.

pub mod tests;
pub mod tracker;
pub mod types;

pub use tracker::IssueTracker;
pub use types::{ExternalIssue, IssueState, SyncError, SyncResult, TrackerKind};
