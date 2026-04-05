//! Session Resume Support
//!
//! Persists agent conversation state to allow resuming after crashes
//! or intentional stops. On retry, full context is restored instead
//! of starting fresh.
//!
//! Snapshots are stored as JSON files in the configured snapshot directory,
//! one file per session: `{snapshot_dir}/{session_id}.json`
//!
//! ## Compaction-Aware Resume
//!
//! For long-running sessions, use `CompactResume` to resume from a compact
//! summary boundary plus recent events, avoiding the need to carry
//! excessive historical context.

pub mod compaction;
pub mod event_log;
pub mod manager;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use compaction::{
    CompactResume, CompactedSnapshot, CompactionBoundary, MAX_COMPACT_MESSAGES, MAX_TAIL_EVENTS,
};
pub use event_log::{EventCategory, EventData, EventLog, EventSeverity, SessionEvent};
pub use manager::ResumeManager;
pub use types::{
    ResumeError, ResumeResult, SerializedMessage, SerializedToolCall, SessionSnapshot, Snapshot,
    SnapshotInfo, ToolRecord,
};
