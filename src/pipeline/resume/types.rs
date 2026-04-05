//! Session Resume Types
//!
//! Data structures for session snapshots and resume support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::event_log::EventLog;

/// Snapshot of an agent session that can be restored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Session ID being snapshotted.
    pub session_id: String,
    /// Task ID this session is working on.
    pub task_id: String,
    /// When the snapshot was taken.
    pub snapshot_at: DateTime<Utc>,
    /// Current conversation messages (serialized as JSON).
    pub messages: Vec<SerializedMessage>,
    /// Current phase of the task.
    pub current_phase: String,
    /// Files that have been modified in this session.
    pub modified_files: Vec<String>,
    /// Tool execution history.
    pub tool_history: Vec<ToolRecord>,
    /// Checkpoint metadata.
    pub checkpoint_note: Option<String>,
    /// Internal event log for session replay.
    #[serde(default)]
    pub event_log: Option<EventLog>,
}

/// Serialized message (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMessage {
    /// One of "user", "assistant", "system", "tool".
    pub role: String,
    /// Message content.
    pub content: String,
    /// Tool calls embedded in this message, if any.
    pub tool_calls: Option<Vec<SerializedToolCall>>,
}

/// Serialized tool call inside a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedToolCall {
    /// Tool call identifier.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool input as arbitrary JSON.
    pub input: serde_json::Value,
}

/// Record of a single tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRecord {
    /// Name of the tool that was invoked.
    pub tool_name: String,
    /// Short summary of the input.
    pub input_summary: String,
    /// Whether the invocation succeeded.
    pub success: bool,
    /// When the tool was invoked.
    pub timestamp: DateTime<Utc>,
}

/// Summary returned after a resume operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// The restored session ID.
    pub session_id: String,
    /// Number of messages restored.
    pub messages_restored: usize,
    /// Number of tool records restored.
    pub tools_restored: usize,
    /// Number of files tracked in the snapshot.
    pub files_tracked: usize,
    /// Human-readable age of the snapshot.
    pub snapshot_age: String,
}

/// Light metadata about a snapshot, used for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// Session ID.
    pub session_id: String,
    /// Task ID.
    pub task_id: String,
    /// When the snapshot was taken.
    pub snapshot_at: DateTime<Utc>,
    /// Number of messages stored.
    pub message_count: usize,
    /// Number of modified files tracked.
    pub file_count: usize,
    /// Number of events in the event log.
    pub event_count: usize,
}

/// Errors that can occur during snapshot operations.
#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    /// An I/O error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A serialization / deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A requested snapshot was not found.
    #[error("Snapshot not found: {0}")]
    NotFound(String),
}

/// Unified snapshot type that can hold either a raw session snapshot or a compact resume.
///
/// This enum enables backward compatibility: old `SessionSnapshot` data is loaded as-is,
/// while new compact resume data is loaded with full compaction awareness.
///
/// When saving, you can choose between:
/// - `Snapshot::Full(snapshot)` - traditional full snapshot (backward compatible)
/// - `Snapshot::Compact(resume)` - compact resume with boundary + tail events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Snapshot {
    /// Full session snapshot with all messages and event log.
    Full(SessionSnapshot),
    /// Compact resume with boundary summary and tail events.
    Compact(crate::pipeline::resume::compaction::CompactResume),
}

impl Snapshot {
    /// Get the session ID from this snapshot.
    pub fn session_id(&self) -> &str {
        match self {
            Snapshot::Full(s) => &s.session_id,
            Snapshot::Compact(c) => c.boundary.session_id(),
        }
    }

    /// Get the task ID from this snapshot.
    pub fn task_id(&self) -> &str {
        match self {
            Snapshot::Full(s) => &s.task_id,
            Snapshot::Compact(c) => c.boundary.task_id(),
        }
    }

    /// Check if this is a compact resume.
    pub fn is_compact(&self) -> bool {
        matches!(self, Snapshot::Compact(_))
    }

    /// Check if this is a full snapshot.
    pub fn is_full(&self) -> bool {
        matches!(self, Snapshot::Full(_))
    }

    /// Convert to session snapshot if possible, returning None for compact.
    pub fn as_full(&self) -> Option<&SessionSnapshot> {
        match self {
            Snapshot::Full(s) => Some(s),
            Snapshot::Compact(_) => None,
        }
    }

    /// Convert to compact resume if possible, returning None for full.
    pub fn as_compact(&self) -> Option<&crate::pipeline::resume::compaction::CompactResume> {
        match self {
            Snapshot::Full(_) => None,
            Snapshot::Compact(c) => Some(c),
        }
    }

    /// Convert this snapshot to a `SessionSnapshot`.
    ///
    /// For full snapshots, returns a clone.
    /// For compact snapshots, reconstructs a session snapshot from the boundary
    /// and tail events. This enables the restore flow to work with both formats.
    pub fn to_session_snapshot(&self) -> SessionSnapshot {
        match self {
            Snapshot::Full(s) => s.clone(),
            Snapshot::Compact(c) => {
                let boundary = &c.boundary;
                let tail = &c.tail_events;

                // Reconstruct event log from tail events
                let mut event_log = EventLog::new(&boundary.session_id);
                for event in tail {
                    event_log.record(event.clone());
                }

                // Extract tool records from events
                let tool_history: Vec<ToolRecord> = tail
                    .iter()
                    .filter_map(|e| {
                        if matches!(e.category, super::event_log::EventCategory::ToolExecution) {
                            // Try to extract tool info from structured data
                            let (tool_name, success) =
                                if let super::event_log::EventData::Tool(tool_data) = &e.data {
                                    (tool_data.tool_name.clone(), tool_data.success)
                                } else {
                                    (e.name.clone(), true)
                                };
                            Some(ToolRecord {
                                tool_name,
                                input_summary: String::new(),
                                success,
                                timestamp: e.timestamp,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                SessionSnapshot {
                    session_id: boundary.session_id.clone(),
                    task_id: boundary.task_id.clone(),
                    snapshot_at: boundary.created_at,
                    messages: boundary.recent_messages.clone(),
                    current_phase: boundary.phase.clone(),
                    modified_files: boundary.modified_files.clone(),
                    tool_history,
                    checkpoint_note: boundary.compaction_note.clone(),
                    event_log: Some(event_log),
                }
            }
        }
    }
}
