//! Compaction-Aware Session Resume
//!
//! Provides a mechanism for resuming from a compact session summary boundary
//! plus recent events, rather than relying on full raw history.
//!
//! Design:
//! - `CompactionBoundary` stores the compact summary of session state
//! - After compaction, recent events are kept separately from the compact boundary
//! - Resume reconstructs session state from boundary + tail events
//! - Fallback to full snapshot when no compaction exists

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::event_log::{EventLog, SessionEvent};

/// Maximum number of messages to retain in the compact boundary.
pub const MAX_COMPACT_MESSAGES: usize = 20;

/// Maximum number of tail events to retain after compaction.
pub const MAX_TAIL_EVENTS: usize = 100;

/// Keep the last N items from a vector, preserving order.
fn keep_last_n<T>(items: Vec<T>, n: usize) -> Vec<T> {
    let len = items.len();
    if len <= n {
        items
    } else {
        items.into_iter().skip(len - n).collect()
    }
}

/// Compaction boundary - a compact snapshot of session state at a point in time.
///
/// This represents the "trusted compact summary" that the agent can safely
/// resume from without replaying all historical context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionBoundary {
    /// Session ID.
    pub session_id: String,
    /// Task ID.
    pub task_id: String,
    /// When the boundary was created.
    pub created_at: DateTime<Utc>,
    /// Last event sequence number at compaction time.
    pub last_seq: u64,
    /// Current phase at compaction.
    pub phase: String,
    /// Current iteration at compaction.
    pub iteration: u32,
    /// Merge readiness state at compaction.
    pub merge_ready: bool,
    /// Merge readiness summary string.
    pub merge_summary: String,
    /// Files modified up to this point.
    pub modified_files: Vec<String>,
    /// Recent messages (compact, last N).
    pub recent_messages: Vec<super::types::SerializedMessage>,
    /// QA state summary at compaction.
    pub qa_summary: String,
    /// Event categories present at compaction (for replay reference).
    pub event_categories: Vec<String>,
    /// Optional notes about what happened before compaction.
    pub compaction_note: Option<String>,
}

impl CompactionBoundary {
    /// Create a new compaction boundary from current session state.
    ///
    /// The `last_seq` parameter should be the sequence number of the last event
    /// to include in the boundary summary. Events with `seq > last_seq` will
    /// be kept as tail events in `CompactResume`.
    pub fn new(
        session_id: &str,
        task_id: &str,
        last_seq: u64,
        phase: &str,
        iteration: u32,
        merge_ready: bool,
        merge_summary: &str,
        modified_files: Vec<String>,
        recent_messages: Vec<super::types::SerializedMessage>,
        qa_summary: &str,
        event_log: &EventLog,
        compaction_note: Option<String>,
    ) -> Self {
        let event_categories: Vec<String> = event_log
            .events()
            .iter()
            .filter(|e| e.seq <= last_seq)
            .map(|e| format!("{:?}", e.category))
            .collect();

        Self {
            session_id: session_id.to_string(),
            task_id: task_id.to_string(),
            created_at: Utc::now(),
            last_seq,
            phase: phase.to_string(),
            iteration,
            merge_ready,
            merge_summary: merge_summary.to_string(),
            modified_files,
            recent_messages,
            qa_summary: qa_summary.to_string(),
            event_categories,
            compaction_note,
        }
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the task ID.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Check if this boundary can be used for resumption.
    pub fn is_valid(&self) -> bool {
        !self.session_id.is_empty() && !self.task_id.is_empty()
    }
}

/// Compacted snapshot - the result of compaction operation.
///
/// Contains the compact boundary plus recent tail events that occurred
/// after the boundary was established.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactedSnapshot {
    /// The compaction boundary (compact summary).
    pub boundary: CompactionBoundary,
    /// Events that occurred after the boundary was established.
    pub tail_events: Vec<SessionEvent>,
    /// Total events before compaction (for reference).
    pub events_before_compaction: usize,
    /// Total events after compaction.
    pub events_after_compaction: usize,
}

impl CompactedSnapshot {
    /// Create a new compacted snapshot from a boundary and tail events.
    pub fn new(
        boundary: CompactionBoundary,
        tail_events: Vec<SessionEvent>,
        events_before_compaction: usize,
    ) -> Self {
        let events_after_compaction = tail_events.len();
        Self {
            boundary,
            tail_events,
            events_before_compaction,
            events_after_compaction,
        }
    }

    /// Get total events represented by this compact snapshot.
    pub fn total_events(&self) -> usize {
        self.events_before_compaction + self.events_after_compaction
    }

    /// Check if this is a valid compact snapshot.
    pub fn is_valid(&self) -> bool {
        self.boundary.is_valid()
    }

    /// Reconstruct events for replay from boundary's categories + tail.
    pub fn replay_events(&self) -> Vec<&SessionEvent> {
        self.tail_events
            .iter()
            .filter(|e| e.category.include_in_replay())
            .collect()
    }

    /// Get recent events (last N).
    pub fn recent_tail_events(&self, count: usize) -> Vec<&SessionEvent> {
        let start = self.tail_events.len().saturating_sub(count);
        self.tail_events[start..].iter().collect()
    }
}

/// Session snapshot with optional compaction support.
///
/// When a session has been compacted, the full history is replaced with:
/// - `CompactionBoundary` containing the compact summary
/// - `tail_events` containing events after compaction
///
/// The original `messages` and `event_log` are still available for
/// backward compatibility, but compact snapshots take precedence when present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactResume {
    /// The compaction boundary (compact summary).
    pub boundary: CompactionBoundary,
    /// Events that occurred after the last compaction boundary.
    pub tail_events: Vec<SessionEvent>,
    /// Number of messages before compaction (for reference).
    pub messages_before_compaction: usize,
}

impl CompactResume {
    /// Create a new compact resume from session state.
    ///
    /// The `cutoff_seq` parameter defines the boundary between events that go
    /// into the compact boundary summary (seq <= cutoff_seq) and events that
    /// become tail events (seq > cutoff_seq).
    ///
    /// Typical usage:
    /// 1. Log all events that should be summarized in the boundary
    /// 2. Call `event_log.current_seq()` to get the cutoff point
    /// 3. Continue logging new events (these become the tail)
    /// 4. Call `from_session()` with the captured cutoff_seq
    pub fn from_session(
        session_id: &str,
        task_id: &str,
        cutoff_seq: u64,
        phase: &str,
        iteration: u32,
        merge_ready: bool,
        merge_summary: &str,
        modified_files: Vec<String>,
        messages: Vec<super::types::SerializedMessage>,
        qa_summary: &str,
        event_log: &EventLog,
        compaction_note: Option<String>,
    ) -> Self {
        let events_before = event_log.len();

        // Keep the LAST N messages, not the first N
        let recent_messages = keep_last_n(messages, MAX_COMPACT_MESSAGES);

        let boundary = CompactionBoundary::new(
            session_id,
            task_id,
            cutoff_seq,
            phase,
            iteration,
            merge_ready,
            merge_summary,
            modified_files,
            recent_messages,
            qa_summary,
            event_log,
            compaction_note,
        );

        // Tail events: everything after the cutoff_seq
        let tail_events: Vec<SessionEvent> = event_log
            .events()
            .iter()
            .filter(|e| e.seq > cutoff_seq)
            .cloned()
            .collect();

        Self {
            boundary,
            tail_events,
            messages_before_compaction: events_before,
        }
    }

    /// Create a compact resume with all events in the tail.
    ///
    /// This is useful when you want to capture a boundary at the current point
    /// but keep all subsequent events as tail. The boundary's `last_seq` will
    /// be set to `current_seq()`.
    ///
    /// WARNING: This creates an empty tail! Use `from_session()` with an explicit
    /// `cutoff_seq` if you need non-empty tail events.
    pub fn at_point(
        session_id: &str,
        task_id: &str,
        phase: &str,
        iteration: u32,
        merge_ready: bool,
        merge_summary: &str,
        modified_files: Vec<String>,
        messages: Vec<super::types::SerializedMessage>,
        qa_summary: &str,
        event_log: &EventLog,
        compaction_note: Option<String>,
    ) -> Self {
        let cutoff_seq = event_log.current_seq();
        Self::from_session(
            session_id,
            task_id,
            cutoff_seq,
            phase,
            iteration,
            merge_ready,
            merge_summary,
            modified_files,
            messages,
            qa_summary,
            event_log,
            compaction_note,
        )
    }

    /// Get the boundary.
    pub fn boundary(&self) -> &CompactionBoundary {
        &self.boundary
    }

    /// Get tail events.
    pub fn tail_events(&self) -> &[SessionEvent] {
        &self.tail_events
    }

    /// Get replay-ready events.
    pub fn replay_events(&self) -> Vec<&SessionEvent> {
        self.tail_events
            .iter()
            .filter(|e| e.category.include_in_replay())
            .collect()
    }

    /// Get recent tail events (last N).
    pub fn recent_tail_events(&self, count: usize) -> Vec<&SessionEvent> {
        let start = self.tail_events.len().saturating_sub(count);
        self.tail_events[start..].iter().collect()
    }

    /// Check if this is a valid compact resume.
    pub fn is_valid(&self) -> bool {
        self.boundary.is_valid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::resume::types::SerializedMessage;

    fn make_test_event_log() -> EventLog {
        let log = EventLog::new("test-session");
        log
    }

    fn make_test_messages() -> Vec<SerializedMessage> {
        vec![
            SerializedMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_calls: None,
            },
            SerializedMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
                tool_calls: None,
            },
        ]
    }

    #[test]
    fn test_compaction_boundary_creation() {
        let mut log = make_test_event_log();
        log.log_agent_start("sess-1", Some(1));
        log.log_tool_start("tool-1", "Read");

        // After logging 2 events, current_seq is 1
        let boundary_seq = log.current_seq();

        let boundary = CompactionBoundary::new(
            "sess-1",
            "task-1",
            boundary_seq,
            "implement",
            2,
            true,
            "All checks passed",
            vec!["src/main.rs".to_string()],
            make_test_messages(),
            "In review phase",
            &log,
            Some("Mid-session checkpoint".to_string()),
        );

        assert_eq!(boundary.session_id, "sess-1");
        assert_eq!(boundary.task_id, "task-1");
        assert_eq!(boundary.phase, "implement");
        assert_eq!(boundary.iteration, 2);
        assert_eq!(boundary.last_seq, boundary_seq);
        assert!(boundary.merge_ready);
        assert!(boundary.is_valid());
    }

    #[test]
    fn test_compacted_snapshot_creation() {
        let mut log = EventLog::new("test");
        log.log_agent_start("sess-1", Some(1));
        log.log_tool_start("tool-1", "Read");

        // Capture boundary at this point
        let boundary_seq = log.current_seq();

        log.log_tool_end("tool-1", "Read", true, Some(100));

        // Boundary includes events up to seq <= boundary_seq
        let boundary = CompactionBoundary::new(
            "sess-1",
            "task-1",
            boundary_seq,
            "plan",
            1,
            false,
            "Pending review",
            vec![],
            vec![],
            "Initial state",
            &log,
            None,
        );

        let events_before = log.len();
        log.log_phase_transition(Some("plan"), "implement", "task-1");

        // Tail is events after boundary_seq
        let tail: Vec<SessionEvent> = log
            .events()
            .iter()
            .filter(|e| e.seq > boundary_seq)
            .cloned()
            .collect();

        let compact = CompactedSnapshot::new(boundary, tail, events_before);

        assert!(compact.is_valid());
        assert_eq!(compact.events_before_compaction, events_before);
        assert_eq!(compact.events_after_compaction, 2);
        assert_eq!(compact.total_events(), events_before + 2);
    }

    #[test]
    fn test_compact_resume_from_session() {
        let mut log = EventLog::new("sess-compact");

        // Phase 1: Log events that will be in the boundary
        log.log_agent_start("sess-compact", Some(1));
        log.log_tool_start("tool-1", "Read");

        // Capture boundary at this point
        let boundary_seq = log.current_seq();

        // Phase 2: Log events that will become the tail
        log.log_phase_transition(None, "Plan", "task-x");
        log.log_qa_state("in_review", 1, 0);

        let pre_events = log.len();

        let compact = CompactResume::from_session(
            "sess-compact",
            "task-x",
            boundary_seq,
            "plan",
            1,
            false,
            "Phase transition",
            vec!["file.rs".to_string()],
            make_test_messages(),
            "Planning phase",
            &log,
            Some("Pre-compaction checkpoint".to_string()),
        );

        assert!(compact.is_valid());
        assert_eq!(compact.boundary.session_id, "sess-compact");
        assert_eq!(compact.boundary.last_seq, boundary_seq);
        // Tail should have exactly the 2 events logged after boundary_seq
        assert_eq!(compact.tail_events.len(), 2);
        assert_eq!(compact.messages_before_compaction, pre_events);

        // Verify tail contains the correct events
        assert!(compact.tail_events.iter().any(|e| e.name.contains("phase")));
        assert!(compact
            .tail_events
            .iter()
            .any(|e| e.name.contains("qa_state")));
    }

    #[test]
    fn test_compact_resume_retains_tail_after_compaction() {
        // This test verifies the core fix: tail events are properly retained
        let mut log = EventLog::new("tail-retain-test");

        // Log some initial events
        for i in 0..5 {
            log.log(
                format!("init-{}", i),
                crate::pipeline::resume::EventCategory::AgentLifecycle,
                crate::pipeline::resume::EventData::empty(),
            );
        }

        // Capture the boundary point
        let boundary_seq = log.current_seq();

        // Log more events after the boundary
        for i in 0..3 {
            log.log(
                format!("tail-{}", i),
                crate::pipeline::resume::EventCategory::ToolExecution,
                crate::pipeline::resume::EventData::empty(),
            );
        }

        let compact = CompactResume::from_session(
            "tail-retain-test",
            "task-1",
            boundary_seq,
            "test",
            1,
            false,
            "Testing",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        // Core assertion: tail should NOT be empty
        assert!(
            !compact.tail_events.is_empty(),
            "Tail events should be retained"
        );
        assert_eq!(compact.tail_events.len(), 3);

        // Verify all tail events are after boundary
        for event in &compact.tail_events {
            assert!(
                event.seq > boundary_seq,
                "All tail events should have seq > boundary_seq"
            );
        }

        // Verify boundary only includes events up to boundary_seq
        assert!(compact.boundary.last_seq <= boundary_seq);
    }

    #[test]
    fn test_compact_resume_replay_events() {
        let mut log = EventLog::new("replay-test");
        log.log_agent_start("sess-1", Some(1));
        log.log_tool_start("tool-1", "Bash"); // System event
        log.log_tool_end("tool-1", "Bash", true, Some(50));

        // Capture boundary after system events
        let boundary_seq = log.current_seq();

        // Log events that will become tail
        log.log_phase_transition(None, "Implement", "task-1");

        let compact = CompactResume::from_session(
            "sess-1",
            "task-1",
            boundary_seq,
            "implement",
            1,
            false,
            "In progress",
            vec![],
            vec![],
            "Testing",
            &log,
            None,
        );

        let replay = compact.replay_events();
        // Should exclude system events
        assert!(replay.iter().all(|e| e.category.include_in_replay()));
    }

    #[test]
    fn test_compacted_snapshot_recent_tail() {
        let mut log = EventLog::new("tail-test");
        for i in 0..10 {
            log.log(
                format!("e{}", i),
                crate::pipeline::resume::EventCategory::System,
                crate::pipeline::resume::EventData::empty(),
            );
        }

        // Create boundary including all 10 events
        let boundary_seq = 9;

        let boundary = CompactionBoundary::new(
            "sess-1",
            "task-1",
            boundary_seq,
            "test",
            1,
            true,
            "ok",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        // No tail events since we captured everything
        let tail: Vec<SessionEvent> = vec![];
        let compact = CompactedSnapshot::new(boundary, tail, 10);

        // With empty tail, recent_tail_events returns empty
        let recent = compact.recent_tail_events(3);
        assert_eq!(recent.len(), 0);
    }

    #[test]
    fn test_compacted_snapshot_with_tail() {
        let mut log = EventLog::new("tail-with-events");

        // Log 5 events
        for i in 0..5 {
            log.log(
                format!("init-{}", i),
                crate::pipeline::resume::EventCategory::AgentLifecycle,
                crate::pipeline::resume::EventData::empty(),
            );
        }

        // Boundary at event 4 (seq 4)
        let boundary_seq = 4;

        // Log 5 more events as tail
        for i in 0..5 {
            log.log(
                format!("tail-{}", i),
                crate::pipeline::resume::EventCategory::ToolExecution,
                crate::pipeline::resume::EventData::empty(),
            );
        }

        let boundary = CompactionBoundary::new(
            "sess-1",
            "task-1",
            boundary_seq,
            "test",
            1,
            true,
            "ok",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        let tail: Vec<SessionEvent> = log
            .events()
            .iter()
            .filter(|e| e.seq > boundary_seq)
            .cloned()
            .collect();
        let compact = CompactedSnapshot::new(boundary, tail, 5);

        assert_eq!(compact.recent_tail_events(3).len(), 3);
    }

    #[test]
    fn test_compaction_boundary_invalid() {
        let boundary = CompactionBoundary {
            session_id: String::new(),
            task_id: "task-1".to_string(),
            created_at: Utc::now(),
            last_seq: 0,
            phase: "test".to_string(),
            iteration: 1,
            merge_ready: false,
            merge_summary: String::new(),
            modified_files: vec![],
            recent_messages: vec![],
            qa_summary: String::new(),
            event_categories: vec![],
            compaction_note: None,
        };

        assert!(!boundary.is_valid());
    }

    #[test]
    fn test_compacted_snapshot_empty_tail() {
        let log = make_test_event_log();

        let boundary = CompactionBoundary::new(
            "sess-1",
            "task-1",
            0, // last_seq: empty log, so boundary at seq 0
            "done",
            1,
            true,
            "All good",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        let compact = CompactedSnapshot::new(boundary, vec![], 5);
        assert!(compact.is_valid());
        assert!(compact.tail_events.is_empty());
        assert_eq!(compact.total_events(), 5);
    }

    #[test]
    fn test_compact_resume_at_point_creates_empty_tail() {
        // This test verifies that at_point() correctly creates an empty tail
        // (as documented in its warning)
        let mut log = EventLog::new("at-point-test");
        log.log_agent_start("sess-1", Some(1));
        log.log_phase_transition(None, "Plan", "task-1");

        let compact = CompactResume::at_point(
            "sess-1",
            "task-1",
            "plan",
            1,
            false,
            "Testing at_point",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        // at_point() captures current state, so tail should be empty
        assert!(
            compact.tail_events.is_empty(),
            "at_point() should create empty tail"
        );
        assert_eq!(compact.boundary.last_seq, log.current_seq());
    }

    #[test]
    fn test_compact_resume_empty_tail_when_no_post_boundary_events() {
        // Verify that empty tail only occurs when there really are no post-boundary events
        let mut log = EventLog::new("no-post-events");

        // Log some events
        log.log_agent_start("sess-1", Some(1));
        log.log_tool_start("tool-1", "Read");

        // Capture boundary at current point (no more events logged after)
        let boundary_seq = log.current_seq();

        // No additional events logged - tail will be empty
        let compact = CompactResume::from_session(
            "sess-1",
            "task-1",
            boundary_seq,
            "test",
            1,
            false,
            "Testing",
            vec![],
            vec![],
            "",
            &log,
            None,
        );

        // Tail is empty because we captured at the last event
        assert!(compact.tail_events.is_empty());
    }

    #[test]
    fn test_boundary_seq_determines_tail_content() {
        // Verify that different boundary_seq values correctly split events
        let mut log = EventLog::new("seq-split-test");

        log.log_agent_start("sess-1", Some(1)); // seq 0
        log.log_tool_start("tool-1", "Read"); // seq 1
        log.log_tool_end("tool-1", "Read", true, None); // seq 2
        log.log_phase_transition(None, "Plan", "task-1"); // seq 3

        // Create compact with boundary at seq 1 (includes events 0 and 1)
        let compact1 = CompactResume::from_session(
            "sess-1",
            "task-1",
            1,
            "test",
            1,
            false,
            "",
            vec![],
            vec![],
            "",
            &log,
            None,
        );
        assert_eq!(compact1.tail_events.len(), 2); // events seq 2, 3

        // Create compact with boundary at seq 2 (includes events 0, 1, 2)
        let compact2 = CompactResume::from_session(
            "sess-1",
            "task-1",
            2,
            "test",
            1,
            false,
            "",
            vec![],
            vec![],
            "",
            &log,
            None,
        );
        assert_eq!(compact2.tail_events.len(), 1); // event seq 3

        // Create compact with boundary at seq 3 (includes all events)
        let compact3 = CompactResume::from_session(
            "sess-1",
            "task-1",
            3,
            "test",
            1,
            false,
            "",
            vec![],
            vec![],
            "",
            &log,
            None,
        );
        assert_eq!(compact3.tail_events.len(), 0); // no tail
    }

    #[test]
    fn test_compact_resume_keeps_latest_messages() {
        // Verify compaction keeps the LAST N messages, not the first N
        let log = EventLog::new("msg-test");

        // Create 30 messages (more than MAX_COMPACT_MESSAGES = 20)
        let messages: Vec<SerializedMessage> = (0..30)
            .map(|i| SerializedMessage {
                role: "user".to_string(),
                content: format!("Message {}", i),
                tool_calls: None,
            })
            .collect();

        let compact = CompactResume::from_session(
            "msg-test",
            "task-1",
            0,
            "test",
            1,
            false,
            "Testing",
            vec![],
            messages,
            "",
            &log,
            None,
        );

        // Should keep messages 10-29 (last 20), not 0-19
        assert_eq!(compact.boundary.recent_messages.len(), 20);
        assert_eq!(compact.boundary.recent_messages[0].content, "Message 10");
        assert_eq!(compact.boundary.recent_messages[19].content, "Message 29");
    }

    #[test]
    fn test_compact_resume_short_session_keeps_all_messages() {
        // Verify that sessions shorter than MAX_COMPACT_MESSAGES keep all messages
        let log = EventLog::new("short-test");

        let messages: Vec<SerializedMessage> = (0..5)
            .map(|i| SerializedMessage {
                role: "user".to_string(),
                content: format!("Msg {}", i),
                tool_calls: None,
            })
            .collect();

        let compact = CompactResume::from_session(
            "short-test",
            "task-1",
            0,
            "test",
            1,
            false,
            "Testing",
            vec![],
            messages,
            "",
            &log,
            None,
        );

        // Should keep all 5 messages
        assert_eq!(compact.boundary.recent_messages.len(), 5);
        assert_eq!(compact.boundary.recent_messages[0].content, "Msg 0");
        assert_eq!(compact.boundary.recent_messages[4].content, "Msg 4");
    }

    #[test]
    fn test_compact_resume_message_order_preserved() {
        // Verify message order is preserved after keeping last N
        let log = EventLog::new("order-test");

        let messages: Vec<SerializedMessage> = (0..25)
            .map(|i| SerializedMessage {
                role: if i % 2 == 0 {
                    "user".to_string()
                } else {
                    "assistant".to_string()
                },
                content: format!("Msg{}", i),
                tool_calls: None,
            })
            .collect();

        let compact = CompactResume::from_session(
            "order-test",
            "task-1",
            0,
            "test",
            1,
            false,
            "Testing",
            vec![],
            messages,
            "",
            &log,
            None,
        );

        // Verify order is preserved
        let msgs = &compact.boundary.recent_messages;
        assert_eq!(msgs.len(), 20);
        assert_eq!(msgs[0].content, "Msg5");
        assert_eq!(msgs[1].content, "Msg6");
        assert_eq!(msgs[19].content, "Msg24");

        // Verify alternating roles are preserved
        assert_eq!(msgs[0].role, "assistant");
        assert_eq!(msgs[1].role, "user");
        assert_eq!(msgs[2].role, "assistant");
    }

    #[test]
    fn test_keep_last_n_helper() {
        // Test the keep_last_n helper function directly
        let items: Vec<i32> = (0..10).collect();

        // Less than max - should return all
        assert_eq!(
            keep_last_n(items.clone(), 15),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );

        // Equal to max - should return all
        assert_eq!(
            keep_last_n(items.clone(), 10),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );

        // More than max - should return last N
        assert_eq!(keep_last_n(items.clone(), 3), vec![7, 8, 9]);
        assert_eq!(keep_last_n(items.clone(), 5), vec![5, 6, 7, 8, 9]);
        assert_eq!(keep_last_n(items, 0), Vec::<i32>::new());
    }

    #[test]
    fn test_compact_resume_exact_max_messages() {
        // Verify behavior when session has exactly MAX_COMPACT_MESSAGES
        let log = EventLog::new("exact-test");

        let messages: Vec<SerializedMessage> = (0..20)
            .map(|i| SerializedMessage {
                role: "user".to_string(),
                content: format!("Msg{}", i),
                tool_calls: None,
            })
            .collect();

        let compact = CompactResume::from_session(
            "exact-test",
            "task-1",
            0,
            "test",
            1,
            false,
            "Testing",
            vec![],
            messages,
            "",
            &log,
            None,
        );

        // Should keep all 20 messages
        assert_eq!(compact.boundary.recent_messages.len(), 20);
        assert_eq!(compact.boundary.recent_messages[0].content, "Msg0");
        assert_eq!(compact.boundary.recent_messages[19].content, "Msg19");
    }
}
