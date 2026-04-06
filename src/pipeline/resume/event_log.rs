//! Session Event Log
//!
//! Provides a bounded, structured internal event log for session replay and recovery.
//! Captures meaningful lifecycle events that help resumed sessions reconstruct
//! recent runtime context without replaying the full conversation.
//!
//! Events are stored in-memory and persisted as part of session snapshots.
//! The log is bounded to prevent unbounded growth.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum number of events to retain in the log.
pub const MAX_EVENT_LOG_SIZE: usize = 1000;

/// Category of event for filtering and replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// Agent lifecycle events (start, stop, error).
    AgentLifecycle,
    /// Tool execution events (start, end, success, failure).
    ToolExecution,
    /// Subagent/child-agent events (spawn, completion).
    Subagent,
    /// Pipeline phase transitions.
    PhaseTransition,
    /// QA loop and review state changes.
    QaState,
    /// General state transitions.
    StateChange,
    /// System-level events (checkpoint, snapshot, etc.).
    System,
}

impl EventCategory {
    /// Returns true if this category should be included in replay by default.
    pub fn include_in_replay(&self) -> bool {
        match self {
            EventCategory::AgentLifecycle
            | EventCategory::ToolExecution
            | EventCategory::Subagent
            | EventCategory::PhaseTransition
            | EventCategory::QaState
            | EventCategory::StateChange => true,
            EventCategory::System => false,
        }
    }
}

/// Severity level for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

/// Structured event for the internal log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Unique event sequence number.
    pub seq: u64,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Event category for filtering.
    pub category: EventCategory,
    /// Event name/verb.
    pub name: String,
    /// Event-specific payload data.
    pub data: EventData,
    /// Severity level.
    pub severity: EventSeverity,
    /// Optional parent sequence for correlating related events.
    pub parent_seq: Option<u64>,
}

impl SessionEvent {
    /// Create a new event with auto-assigned sequence.
    pub fn new(seq: u64, name: String, category: EventCategory, data: EventData) -> Self {
        Self {
            seq,
            timestamp: Utc::now(),
            category,
            name,
            data,
            severity: EventSeverity::Info,
            parent_seq: None,
        }
    }

    /// Create a debug-level event.
    pub fn debug(seq: u64, name: String, category: EventCategory, data: EventData) -> Self {
        Self {
            seq,
            timestamp: Utc::now(),
            category,
            name,
            data,
            severity: EventSeverity::Debug,
            parent_seq: None,
        }
    }

    /// Create an error-level event.
    pub fn error(seq: u64, name: String, category: EventCategory, data: EventData) -> Self {
        Self {
            seq,
            timestamp: Utc::now(),
            category,
            name,
            data,
            severity: EventSeverity::Error,
            parent_seq: None,
        }
    }

    /// Set parent sequence for correlating events (e.g., tool start → tool end).
    pub fn with_parent(mut self, parent_seq: u64) -> Self {
        self.parent_seq = Some(parent_seq);
        self
    }

    /// Check if this event matches a category filter.
    pub fn matches_category(&self, filter: &[EventCategory]) -> bool {
        filter.is_empty() || filter.contains(&self.category)
    }
}

/// Event payload data - structured variants for common event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    /// Empty payload for simple events.
    Empty,
    /// Key-value pairs for structured data.
    Map(std::collections::HashMap<String, serde_json::Value>),
    /// Tool execution data.
    Tool(ToolEventData),
    /// Agent lifecycle data.
    Agent(AgentEventData),
    /// Phase transition data.
    Phase(PhaseEventData),
    /// QA state change data.
    Qa(QaEventData),
    /// Subagent event data.
    Subagent(SubagentEventData),
}

impl EventData {
    /// Create empty event data.
    pub fn empty() -> Self {
        EventData::Empty
    }

    /// Create tool event data.
    pub fn tool(tool_name: &str, tool_id: &str, success: bool) -> Self {
        EventData::Tool(ToolEventData {
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
            success,
            error_message: None,
            duration_ms: None,
        })
    }

    /// Create agent lifecycle data.
    pub fn agent(session_id: &str, state: &str) -> Self {
        EventData::Agent(AgentEventData {
            session_id: session_id.to_string(),
            state: state.to_string(),
            iteration: None,
        })
    }

    /// Create phase transition data.
    pub fn phase(from: Option<&str>, to: &str, task_id: &str) -> Self {
        EventData::Phase(PhaseEventData {
            from_phase: from.map(String::from),
            to_phase: to.to_string(),
            task_id: task_id.to_string(),
        })
    }

    /// Create QA event data.
    pub fn qa(state: &str, iteration: u32, blockers: usize) -> Self {
        EventData::Qa(QaEventData {
            state: state.to_string(),
            iteration,
            blockers,
            message: None,
        })
    }

    /// Create subagent event data.
    pub fn subagent(subagent_id: &str, task: &str, status: &str) -> Self {
        EventData::Subagent(SubagentEventData {
            subagent_id: subagent_id.to_string(),
            task: task.to_string(),
            status: status.to_string(),
        })
    }
}

/// Tool execution event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEventData {
    pub tool_name: String,
    pub tool_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Agent lifecycle event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEventData {
    pub session_id: String,
    pub state: String,
    pub iteration: Option<u32>,
}

/// Phase transition event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseEventData {
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub task_id: String,
}

/// QA state change event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaEventData {
    pub state: String,
    pub iteration: u32,
    pub blockers: usize,
    pub message: Option<String>,
}

/// Subagent event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentEventData {
    pub subagent_id: String,
    pub task: String,
    pub status: String,
}

/// Bounded internal event log for session replay.
///
/// Maintains a sliding window of recent events with configurable maximum size.
/// Events older than the window are dropped to prevent unbounded memory growth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    /// Bounded event buffer (newest last).
    events: Vec<SessionEvent>,
    /// Maximum events to retain.
    max_size: usize,
    /// Running sequence counter.
    next_seq: u64,
    /// Session ID this log belongs to.
    session_id: String,
}

impl EventLog {
    /// Create a new event log for a session.
    pub fn new(session_id: &str) -> Self {
        Self {
            events: Vec::with_capacity(MAX_EVENT_LOG_SIZE),
            max_size: MAX_EVENT_LOG_SIZE,
            next_seq: 0,
            session_id: session_id.to_string(),
        }
    }

    /// Create with custom max size.
    pub fn with_max_size(session_id: &str, max_size: usize) -> Self {
        Self {
            events: Vec::with_capacity(max_size),
            max_size,
            next_seq: 0,
            session_id: session_id.to_string(),
        }
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get current sequence number.
    pub fn current_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }

    /// Record a new event, dropping oldest if at capacity.
    pub fn record(&mut self, event: SessionEvent) {
        if self.events.len() >= self.max_size {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Record an event with auto-assigned sequence.
    pub fn log(&mut self, name: String, category: EventCategory, data: EventData) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        let event = SessionEvent::new(seq, name, category, data);
        self.record(event);
        seq
    }

    /// Record a tool start event, returns sequence for correlation.
    pub fn log_tool_start(&mut self, tool_id: &str, tool_name: &str) -> u64 {
        let mut data = EventData::tool(tool_name, tool_id, true);
        if let EventData::Tool(ref mut t) = data {
            t.success = true;
        }
        self.log(
            format!("tool_start:{}", tool_name),
            EventCategory::ToolExecution,
            data,
        )
    }

    /// Record a tool end event, correlates with start.
    pub fn log_tool_end(
        &mut self,
        tool_id: &str,
        tool_name: &str,
        success: bool,
        duration_ms: Option<u64>,
    ) {
        let mut data = EventData::tool(tool_name, tool_id, success);
        if let EventData::Tool(ref mut t) = data {
            t.success = success;
            t.duration_ms = duration_ms;
        }
        self.log(
            format!("tool_end:{}", tool_name),
            EventCategory::ToolExecution,
            data,
        );
    }

    /// Record agent start.
    pub fn log_agent_start(&mut self, session_id: &str, iteration: Option<u32>) -> u64 {
        let mut data = EventData::agent(session_id, "running");
        if let EventData::Agent(ref mut a) = data {
            a.iteration = iteration;
        }
        self.log(
            "agent_start".to_string(),
            EventCategory::AgentLifecycle,
            data,
        )
    }

    /// Record agent stop.
    pub fn log_agent_stop(&mut self, session_id: &str, state: &str) {
        let data = EventData::agent(session_id, state);
        self.log(
            "agent_stop".to_string(),
            EventCategory::AgentLifecycle,
            data,
        );
    }

    /// Record agent error.
    pub fn log_agent_error(&mut self, session_id: &str, error: &str) {
        let mut data = EventData::agent(session_id, "error");
        if let EventData::Agent(ref mut a) = data {
            a.state = error.to_string();
        }
        let event = SessionEvent::error(
            self.next_seq,
            "agent_error".to_string(),
            EventCategory::AgentLifecycle,
            data,
        );
        self.next_seq += 1;
        self.record(event);
    }

    /// Record phase transition.
    pub fn log_phase_transition(&mut self, from: Option<&str>, to: &str, task_id: &str) {
        let data = EventData::phase(from, to, task_id);
        self.log(
            format!("phase:{}", to),
            EventCategory::PhaseTransition,
            data,
        );
    }

    /// Record QA state change.
    pub fn log_qa_state(&mut self, state: &str, iteration: u32, blockers: usize) {
        let data = EventData::qa(state, iteration, blockers);
        self.log("qa_state".to_string(), EventCategory::QaState, data);
    }

    /// Record subagent spawn.
    pub fn log_subagent_spawn(&mut self, subagent_id: &str, task: &str) -> u64 {
        let data = EventData::subagent(subagent_id, task, "spawned");
        self.log("subagent_spawn".to_string(), EventCategory::Subagent, data)
    }

    /// Record subagent completion.
    pub fn log_subagent_complete(&mut self, subagent_id: &str, task: &str) {
        let data = EventData::subagent(subagent_id, task, "completed");
        self.log(
            "subagent_complete".to_string(),
            EventCategory::Subagent,
            data,
        );
    }

    /// Record checkpoint event.
    pub fn log_checkpoint(&mut self, note: &str) {
        let mut map = std::collections::HashMap::new();
        map.insert("note".to_string(), serde_json::json!(note));
        let data = EventData::Map(map);
        self.log("checkpoint".to_string(), EventCategory::System, data);
    }

    /// Get all events.
    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }

    /// Get events by category filter.
    pub fn events_by_category(&self, categories: &[EventCategory]) -> Vec<&SessionEvent> {
        self.events
            .iter()
            .filter(|e| e.matches_category(categories))
            .collect()
    }

    /// Get events for replay (excludes system events).
    pub fn replay_events(&self) -> Vec<&SessionEvent> {
        self.events
            .iter()
            .filter(|e| e.category.include_in_replay())
            .collect()
    }

    /// Get recent events (last N).
    pub fn recent(&self, count: usize) -> Vec<&SessionEvent> {
        let start = self.events.len().saturating_sub(count);
        self.events[start..].iter().collect()
    }

    /// Get events since a sequence number.
    pub fn since(&self, seq: u64) -> Vec<&SessionEvent> {
        self.events.iter().filter(|e| e.seq > seq).collect()
    }

    /// Get total event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Export events as a summary for debugging.
    pub fn summary(&self) -> EventLogSummary {
        let mut by_category: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_severity: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for event in &self.events {
            *by_category
                .entry(format!("{:?}", event.category))
                .or_insert(0) += 1;
            *by_severity
                .entry(format!("{:?}", event.severity))
                .or_insert(0) += 1;
        }

        EventLogSummary {
            session_id: self.session_id.clone(),
            total_events: self.events.len(),
            next_seq: self.next_seq,
            oldest_seq: self.events.first().map(|e| e.seq),
            newest_seq: self.events.last().map(|e| e.seq),
            by_category,
            by_severity,
        }
    }
}

/// Summary statistics for an event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogSummary {
    pub session_id: String,
    pub total_events: usize,
    pub next_seq: u64,
    pub oldest_seq: Option<u64>,
    pub newest_seq: Option<u64>,
    pub by_category: std::collections::HashMap<String, usize>,
    pub by_severity: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_log_records_events() {
        let mut log = EventLog::new("test-session");
        assert_eq!(log.len(), 0);

        let seq1 = log.log(
            "test_event".to_string(),
            EventCategory::System,
            EventData::empty(),
        );
        assert_eq!(seq1, 0);
        assert_eq!(log.len(), 1);

        let seq2 = log.log(
            "another_event".to_string(),
            EventCategory::AgentLifecycle,
            EventData::empty(),
        );
        assert_eq!(seq2, 1);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_event_log_bounded() {
        let mut log = EventLog::with_max_size("test", 3);
        log.log("e1".to_string(), EventCategory::System, EventData::empty());
        log.log("e2".to_string(), EventCategory::System, EventData::empty());
        log.log("e3".to_string(), EventCategory::System, EventData::empty());
        assert_eq!(log.len(), 3);

        log.log("e4".to_string(), EventCategory::System, EventData::empty());
        assert_eq!(log.len(), 3);
        assert_eq!(log.events[0].name, "e2");
        assert_eq!(log.events[2].name, "e4");
    }

    #[test]
    fn test_event_log_replay_events() {
        let mut log = EventLog::new("test");
        log.log("sys".to_string(), EventCategory::System, EventData::empty());
        log.log(
            "agent".to_string(),
            EventCategory::AgentLifecycle,
            EventData::empty(),
        );
        log.log(
            "tool".to_string(),
            EventCategory::ToolExecution,
            EventData::empty(),
        );

        let replay = log.replay_events();
        assert_eq!(replay.len(), 2);
        assert!(!replay.iter().any(|e| e.name == "sys"));
    }

    #[test]
    fn test_event_log_recent() {
        let mut log = EventLog::new("test");
        for i in 0..10 {
            log.log(format!("e{}", i), EventCategory::System, EventData::empty());
        }

        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].name, "e7");
        assert_eq!(recent[2].name, "e9");
    }

    #[test]
    fn test_event_log_since() {
        let mut log = EventLog::new("test");
        for i in 0..5 {
            log.log(format!("e{}", i), EventCategory::System, EventData::empty());
        }

        let since = log.since(2);
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].name, "e3");
    }

    #[test]
    fn test_tool_events() {
        let mut log = EventLog::new("test");
        let start_seq = log.log_tool_start("tool-123", "Read");
        assert_eq!(start_seq, 0);

        log.log_tool_end("tool-123", "Read", true, Some(50));

        assert_eq!(log.len(), 2);
        assert!(log.events[0].name.contains("tool_start"));
        assert!(log.events[1].name.contains("tool_end"));
    }

    #[test]
    fn test_agent_events() {
        let mut log = EventLog::new("test");
        log.log_agent_start("sess-1", Some(1));
        log.log_agent_stop("sess-1", "completed");

        assert_eq!(log.len(), 2);
        assert_eq!(log.events[0].name, "agent_start");
        assert_eq!(log.events[1].name, "agent_stop");
    }

    #[test]
    fn test_phase_transition() {
        let mut log = EventLog::new("test");
        log.log_phase_transition(None, "Research", "task-1");
        log.log_phase_transition(Some("Research"), "Plan", "task-1");

        assert_eq!(log.len(), 2);
        if let EventData::Phase(p) = &log.events[0].data {
            assert_eq!(p.from_phase, None);
            assert_eq!(p.to_phase, "Research");
        }
    }

    #[test]
    fn test_qa_state() {
        let mut log = EventLog::new("test");
        log.log_qa_state("in_review", 1, 0);
        log.log_qa_state("awaiting_fix", 1, 2);

        assert_eq!(log.len(), 2);
        if let EventData::Qa(q) = &log.events[1].data {
            assert_eq!(q.state, "awaiting_fix");
            assert_eq!(q.blockers, 2);
        }
    }

    #[test]
    fn test_subagent_events() {
        let mut log = EventLog::new("test");
        let seq = log.log_subagent_spawn("sub-1", "fix bug");
        log.log_subagent_complete("sub-1", "fix bug");

        assert_eq!(seq, 0);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_summary() {
        let mut log = EventLog::new("test");
        log.log(
            "e1".to_string(),
            EventCategory::AgentLifecycle,
            EventData::empty(),
        );
        log.log(
            "e2".to_string(),
            EventCategory::AgentLifecycle,
            EventData::empty(),
        );
        log.log(
            "e3".to_string(),
            EventCategory::ToolExecution,
            EventData::empty(),
        );

        let summary = log.summary();
        assert_eq!(summary.total_events, 3);
        assert_eq!(summary.by_category.get("AgentLifecycle"), Some(&2));
        assert_eq!(summary.by_category.get("ToolExecution"), Some(&1));
    }
}
