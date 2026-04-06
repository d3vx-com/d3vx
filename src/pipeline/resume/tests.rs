//! Session Resume Tests

use chrono::Utc;
use tempfile::TempDir;

use super::manager::ResumeManager;
use super::types::*;

fn test_snapshot(session_id: &str, task_id: &str) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session_id.to_string(),
        task_id: task_id.to_string(),
        snapshot_at: Utc::now(),
        messages: vec![
            SerializedMessage {
                role: "user".to_string(),
                content: "Fix the bug".to_string(),
                tool_calls: None,
            },
            SerializedMessage {
                role: "assistant".to_string(),
                content: "Looking into it...".to_string(),
                tool_calls: Some(vec![SerializedToolCall {
                    id: "tc-1".to_string(),
                    name: "Grep".to_string(),
                    input: serde_json::json!({"pattern": "bug"}),
                }]),
            },
        ],
        current_phase: "implement".to_string(),
        modified_files: vec!["src/main.rs".to_string()],
        tool_history: vec![ToolRecord {
            tool_name: "Grep".to_string(),
            input_summary: "pattern: bug".to_string(),
            success: true,
            timestamp: Utc::now(),
        }],
        checkpoint_note: Some("before refactor".to_string()),
        event_log: None,
    }
}

#[tokio::test]
async fn test_save_and_load_snapshot() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let snap = test_snapshot("sess-1", "task-1");
    mgr.save_snapshot(&snap).await.unwrap();

    let loaded = mgr.load_snapshot("sess-1").await.unwrap().unwrap();
    let loaded = loaded.to_session_snapshot();
    assert_eq!(loaded.session_id, "sess-1");
    assert_eq!(loaded.task_id, "task-1");
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.current_phase, "implement");
    assert_eq!(loaded.modified_files, vec!["src/main.rs"]);
    assert_eq!(loaded.tool_history.len(), 1);
    assert_eq!(loaded.checkpoint_note.as_deref(), Some("before refactor"));
}

#[tokio::test]
async fn test_load_nonexistent_returns_none() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let result = mgr.load_snapshot("no-such-session").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_list_snapshots() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    mgr.save_snapshot(&test_snapshot("s1", "t1")).await.unwrap();
    mgr.save_snapshot(&test_snapshot("s2", "t2")).await.unwrap();

    let list = mgr.list_snapshots().await.unwrap();
    assert_eq!(list.len(), 2);
    // Sorted newest first
    assert!(list[0].snapshot_at >= list[1].snapshot_at);
}

#[tokio::test]
async fn test_list_snapshots_empty_dir() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let list = mgr.list_snapshots().await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn test_delete_snapshot() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    mgr.save_snapshot(&test_snapshot("sess-del", "task-del"))
        .await
        .unwrap();

    let deleted = mgr.delete_snapshot("sess-del").await.unwrap();
    assert!(deleted);

    let loaded = mgr.load_snapshot("sess-del").await.unwrap();
    assert!(loaded.is_none());

    // Deleting again returns false
    let deleted_again = mgr.delete_snapshot("sess-del").await.unwrap();
    assert!(!deleted_again);
}

#[tokio::test]
async fn test_load_snapshot_for_task() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    mgr.save_snapshot(&test_snapshot("s-a", "target-task"))
        .await
        .unwrap();
    mgr.save_snapshot(&test_snapshot("s-b", "other-task"))
        .await
        .unwrap();

    let result = mgr.load_snapshot_for_task("target-task").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().task_id, "target-task");
}

#[tokio::test]
async fn test_load_snapshot_for_task_returns_most_recent() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    // Create two snapshots for the same task at different times.
    let mut older = test_snapshot("old-sess", "shared-task");
    older.snapshot_at = Utc::now() - chrono::Duration::hours(2);
    mgr.save_snapshot(&older).await.unwrap();

    let newer = test_snapshot("new-sess", "shared-task");
    mgr.save_snapshot(&newer).await.unwrap();

    let result = mgr
        .load_snapshot_for_task("shared-task")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.session_id, "new-sess");
}

#[tokio::test]
async fn test_cleanup_old_snapshots() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    // Old snapshot
    let mut old = test_snapshot("old-sess", "t1");
    old.snapshot_at = Utc::now() - chrono::Duration::days(10);
    mgr.save_snapshot(&old).await.unwrap();

    // Recent snapshot
    let recent = test_snapshot("recent-sess", "t2");
    mgr.save_snapshot(&recent).await.unwrap();

    let removed = mgr.cleanup_old_snapshots(7).await.unwrap();
    assert_eq!(removed, 1);

    // Old should be gone
    assert!(mgr.load_snapshot("old-sess").await.unwrap().is_none());
    // Recent should remain
    assert!(mgr.load_snapshot("recent-sess").await.unwrap().is_some());
}

#[tokio::test]
async fn test_snapshot_serialization_roundtrip() {
    let snap = test_snapshot("roundtrip", "task-rt");
    let json = serde_json::to_string_pretty(&snap).unwrap();
    let restored: SessionSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.session_id, snap.session_id);
    assert_eq!(restored.messages.len(), snap.messages.len());
    assert_eq!(restored.messages[1].tool_calls.as_ref().unwrap().len(), 1);
    let tc = &restored.messages[1].tool_calls.as_ref().unwrap()[0];
    assert_eq!(tc.name, "Grep");
}

#[tokio::test]
async fn test_atomic_write_no_temp_file_left_behind() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let snap = test_snapshot("atomic-test", "task-atomic");
    mgr.save_snapshot(&snap).await.unwrap();

    let snapshot_path = dir.path().join("atomic-test.json");
    let temp_path = dir.path().join("atomic-test.tmp");

    assert!(snapshot_path.exists(), "snapshot file should exist");
    assert!(
        !temp_path.exists(),
        "temp file should be cleaned up after rename"
    );
}

#[tokio::test]
async fn test_save_overwrites_consistently() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let mut snap1 = test_snapshot("overwrite", "task-ow");
    mgr.save_snapshot(&snap1).await.unwrap();

    snap1.current_phase = "review".to_string();
    mgr.save_snapshot(&snap1).await.unwrap();

    let loaded = mgr.load_snapshot("overwrite").await.unwrap().unwrap();
    assert_eq!(loaded.to_session_snapshot().current_phase, "review");
}

// ========================================================================
// Event Log Persistence Tests
// ========================================================================

use super::event_log::*;

fn test_snapshot_with_events(session_id: &str, task_id: &str) -> SessionSnapshot {
    let mut log = EventLog::new(session_id);
    log.log_agent_start(session_id, Some(1));
    log.log_tool_start("tool-1", "Read");
    log.log_tool_end("tool-1", "Read", true, Some(100));
    log.log_phase_transition(None, "Research", task_id);
    log.log_qa_state("in_review", 1, 0);
    log.log_agent_stop(session_id, "completed");

    SessionSnapshot {
        session_id: session_id.to_string(),
        task_id: task_id.to_string(),
        snapshot_at: Utc::now(),
        messages: vec![],
        current_phase: "research".to_string(),
        modified_files: vec![],
        tool_history: vec![],
        checkpoint_note: None,
        event_log: Some(log),
    }
}

#[tokio::test]
async fn test_snapshot_with_event_log_persists() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let snap = test_snapshot_with_events("event-sess", "event-task");
    assert!(snap.event_log.is_some());
    assert_eq!(snap.event_log.as_ref().unwrap().len(), 6);

    mgr.save_snapshot(&snap).await.unwrap();

    let loaded = mgr.load_snapshot("event-sess").await.unwrap().unwrap();
    let loaded = loaded.to_session_snapshot();
    assert!(loaded.event_log.is_some());
    assert_eq!(loaded.event_log.as_ref().unwrap().len(), 6);
}

#[tokio::test]
async fn test_event_log_replay_events() {
    let mut log = EventLog::new("replay-test");

    log.log_agent_start("sess-1", Some(1));
    log.log_tool_start("tool-1", "Bash");
    log.log_tool_end("tool-1", "Bash", true, Some(50));
    log.log_phase_transition(None, "Plan", "task-1");
    log.log_agent_stop("sess-1", "completed");

    let replay = log.replay_events();
    assert_eq!(replay.len(), 5);
    assert!(replay.iter().all(|e| e.category.include_in_replay()));
}

#[tokio::test]
async fn test_event_log_filter_by_category() {
    let mut log = EventLog::new("filter-test");
    log.log_agent_start("sess-1", None);
    log.log_tool_start("tool-1", "Read");
    log.log_phase_transition(None, "Implement", "task-1");
    log.log_subagent_spawn("sub-1", "fix bug");

    let agent_events = log.events_by_category(&[EventCategory::AgentLifecycle]);
    assert_eq!(agent_events.len(), 1);

    let tool_events = log.events_by_category(&[EventCategory::ToolExecution]);
    assert_eq!(tool_events.len(), 1);

    let multiple = log.events_by_category(&[
        EventCategory::AgentLifecycle,
        EventCategory::PhaseTransition,
    ]);
    assert_eq!(multiple.len(), 2);
}

#[tokio::test]
async fn test_event_log_serialization_roundtrip() {
    let mut log = EventLog::new("serialize-test");
    log.log_agent_start("sess-1", Some(5));
    log.log_tool_start("tool-x", "Write");
    log.log_tool_end("tool-x", "Write", false, Some(200));
    log.log_qa_state("blocked", 2, 3);

    let json = serde_json::to_string(&log).unwrap();
    let restored: EventLog = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.len(), log.len());
    assert_eq!(restored.session_id(), "serialize-test");

    let replay = restored.replay_events();
    assert_eq!(replay.len(), 4);
}

#[tokio::test]
async fn test_event_log_bounded_size() {
    let mut log = EventLog::with_max_size("bounded-test", 3);

    for i in 0..10 {
        log.log(
            format!("event-{}", i),
            EventCategory::System,
            EventData::empty(),
        );
    }

    assert_eq!(log.len(), 3);
    assert_eq!(log.events()[0].name, "event-7");
    assert_eq!(log.events()[2].name, "event-9");
}

#[tokio::test]
async fn test_snapshot_list_includes_event_count() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let snap = test_snapshot_with_events("event-count", "task-count");
    mgr.save_snapshot(&snap).await.unwrap();

    let list = mgr.list_snapshots().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].event_count, 6);
}

#[tokio::test]
async fn test_event_log_since_sequence() {
    let mut log = EventLog::new("seq-test");
    for i in 0..5 {
        log.log(format!("e{}", i), EventCategory::System, EventData::empty());
    }

    let since_2 = log.since(2);
    assert_eq!(since_2.len(), 2);
    assert_eq!(since_2[0].name, "e3");
}

#[tokio::test]
async fn test_event_log_subagent_events() {
    let mut log = EventLog::new("subagent-test");

    let spawn_seq = log.log_subagent_spawn("sub-abc", "implement feature X");
    log.log_subagent_complete("sub-abc", "implement feature X");

    assert_eq!(spawn_seq, 0);
    assert_eq!(log.len(), 2);

    let events = log.events_by_category(&[EventCategory::Subagent]);
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn test_event_log_summary() {
    let mut log = EventLog::new("summary-test");
    log.log_agent_start("sess-1", Some(1));
    log.log_agent_start("sess-1", Some(2));
    log.log_tool_start("tool-1", "Read");
    log.log_phase_transition(None, "Plan", "task-1");

    let summary = log.summary();
    assert_eq!(summary.total_events, 4);
    assert_eq!(summary.by_category.get("AgentLifecycle"), Some(&2));
    assert_eq!(summary.by_category.get("ToolExecution"), Some(&1));
    assert_eq!(summary.by_category.get("PhaseTransition"), Some(&1));
}

#[tokio::test]
async fn test_event_log_qa_state_events() {
    let mut log = EventLog::new("qa-test");

    log.log_qa_state("pending", 0, 0);
    log.log_qa_state("in_review", 1, 0);
    log.log_qa_state("awaiting_fix", 1, 2);
    log.log_qa_state("approved", 1, 0);

    assert_eq!(log.len(), 4);

    let qa_events = log.events_by_category(&[EventCategory::QaState]);
    assert_eq!(qa_events.len(), 4);
}

#[tokio::test]
async fn test_resume_with_event_log_for_replay() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let snap = test_snapshot_with_events("replay-sess", "replay-task");
    mgr.save_snapshot(&snap).await.unwrap();

    let loaded = mgr.load_snapshot("replay-sess").await.unwrap().unwrap();
    let loaded = loaded.to_session_snapshot();

    let log = loaded.event_log.unwrap();
    let replay_events = log.replay_events();

    assert!(!replay_events.is_empty());

    let agent_events: Vec<_> = replay_events
        .iter()
        .filter(|e| e.category == EventCategory::AgentLifecycle)
        .collect();
    assert_eq!(agent_events.len(), 2);

    let tool_events: Vec<_> = replay_events
        .iter()
        .filter(|e| e.category == EventCategory::ToolExecution)
        .collect();
    assert_eq!(tool_events.len(), 2);
}

// ========================================================================
// Compaction-Aware Resume Tests
// ========================================================================

use super::compaction::*;

fn make_test_messages(count: usize) -> Vec<SerializedMessage> {
    (0..count)
        .map(|i| SerializedMessage {
            role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
            content: format!("Message {}", i),
            tool_calls: None,
        })
        .collect()
}

#[tokio::test]
async fn test_compact_resume_persists_with_snapshot() {
    let dir = TempDir::new().unwrap();
    let mgr = ResumeManager::new(dir.path().to_path_buf());
    mgr.initialize().await.unwrap();

    let mut log = EventLog::new("compact-sess");
    log.log_agent_start("compact-sess", Some(1));
    log.log_tool_start("tool-1", "Read");
    log.log_phase_transition(None, "Plan", "task-1");

    let boundary_seq = 1; // Capture boundary after first 2 events
    log.log_phase_transition(None, "Implement", "task-1"); // This becomes tail

    let compact = CompactResume::from_session(
        "compact-sess",
        "task-1",
        boundary_seq,
        "plan",
        1,
        false,
        "Planning phase",
        vec!["src/main.rs".to_string()],
        make_test_messages(5),
        "Initial review",
        &log,
        Some("Pre-compaction checkpoint".to_string()),
    );

    assert!(compact.is_valid());
    assert_eq!(compact.boundary.session_id, "compact-sess");
    assert_eq!(compact.boundary.last_seq, boundary_seq);
    // Should have 2 tail events (Plan transition seq=2 and Implement transition seq=3)
    assert_eq!(compact.tail_events.len(), 2);
}

#[tokio::test]
async fn test_compact_resume_preserves_boundary_state() {
    let mut log = EventLog::new("boundary-test");
    log.log_agent_start("sess-1", Some(5));
    log.log_tool_start("tool-1", "Bash");

    let compact = CompactResume::from_session(
        "sess-1",
        "task-boundary",
        0, // cutoff_seq: boundary at start
        "implement",
        5,
        true,
        "All checks passed - ready to merge",
        vec!["lib.rs".to_string(), "main.rs".to_string()],
        make_test_messages(3),
        "In implementation phase",
        &log,
        None,
    );

    assert_eq!(compact.boundary.phase, "implement");
    assert_eq!(compact.boundary.iteration, 5);
    assert!(compact.boundary.merge_ready);
    assert_eq!(compact.boundary.modified_files.len(), 2);
}

#[tokio::test]
async fn test_compact_resume_replay_after_compaction() {
    let mut log = EventLog::new("replay-compact");
    log.log_agent_start("sess-1", Some(1));
    log.log_tool_start("tool-1", "Read");

    // Events before boundary establishment
    let pre_boundary = log.len();

    // Continue adding events (these become tail)
    log.log_phase_transition(None, "Review", "task-1");
    log.log_qa_state("in_review", 1, 0);
    log.log_agent_stop("sess-1", "completed");

    let compact = CompactResume::from_session(
        "sess-1",
        "task-1",
        pre_boundary as u64, // cutoff_seq
        "review",
        1,
        false,
        "In review",
        vec![],
        vec![],
        "Review phase",
        &log,
        None,
    );

    // Should have pre-boundary events in boundary reference
    assert_eq!(compact.boundary.last_seq, pre_boundary as u64);

    // Tail contains post-boundary events (2 events: seq 3 and 4 pass > 2 filter)
    assert_eq!(compact.tail_events.len(), 2);

    // Replay should work
    let replay = compact.replay_events();
    assert!(!replay.is_empty());
}

#[tokio::test]
async fn test_compacted_snapshot_total_event_count() {
    let mut log = EventLog::new("count-test");
    for i in 0..15 {
        log.log(format!("e{}", i), EventCategory::System, EventData::empty());
    }

    let boundary = CompactionBoundary::new(
        "sess-1",
        "task-1",
        14, // last_seq: include first 15 events (0-14)
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

    let events_before = log.len();
    log.log_phase_transition(None, "Done", "task-1");

    let tail: Vec<SessionEvent> = log.events()[events_before..].to_vec();
    let compact = CompactedSnapshot::new(boundary, tail, events_before);

    assert_eq!(compact.events_before_compaction, 15);
    assert_eq!(compact.events_after_compaction, 1);
    assert_eq!(compact.total_events(), 16);
}

#[tokio::test]
async fn test_compact_resume_max_messages_respected() {
    let mut log = EventLog::new("max-msg");
    log.log_agent_start("sess-1", None);

    let messages = make_test_messages(50); // More than MAX_COMPACT_MESSAGES

    let compact = CompactResume::from_session(
        "sess-1",
        "task-1",
        0, // cutoff_seq
        "test",
        1,
        false,
        "",
        vec![],
        messages,
        "",
        &log,
        None,
    );

    // Should only retain MAX_COMPACT_MESSAGES
    assert!(compact.boundary.recent_messages.len() <= MAX_COMPACT_MESSAGES);
}

#[tokio::test]
async fn test_compact_resume_with_subagent_events() {
    let mut log = EventLog::new("subagent-compact");

    log.log_agent_start("sess-main", Some(1));
    let subagent_seq = log.log_subagent_spawn("sub-abc", "implement feature X");
    log.log_subagent_complete("sub-abc", "implement feature X");

    let compact = CompactResume::from_session(
        "sess-main",
        "task-main",
        0, // cutoff_seq
        "implement",
        1,
        false,
        "Feature in progress",
        vec![],
        vec![],
        "Implementing feature",
        &log,
        None,
    );

    // Should have captured subagent spawn
    assert_eq!(subagent_seq, 1);

    // Tail should have subagent events (all events are in tail since cutoff_seq=0)
    let subagent_events: Vec<_> = compact
        .tail_events
        .iter()
        .filter(|e| e.category == EventCategory::Subagent)
        .collect();
    assert_eq!(subagent_events.len(), 2);
}

#[tokio::test]
async fn test_compact_resume_event_categories_preserved() {
    let mut log = EventLog::new("categories-test");
    log.log_agent_start("sess-1", Some(1));
    log.log_tool_start("tool-1", "Read");
    log.log_phase_transition(None, "Plan", "task-1");
    log.log_qa_state("pending", 0, 0);

    let compact = CompactResume::from_session(
        "sess-1",
        "task-1",
        0, // cutoff_seq
        "plan",
        1,
        false,
        "",
        vec![],
        vec![],
        "",
        &log,
        None,
    );

    // Boundary should record event categories for events with seq <= cutoff_seq (0)
    // Only agent_start (seq=0) is included since cutoff_seq=0 filters seq <= 0
    assert!(compact
        .boundary
        .event_categories
        .contains(&"AgentLifecycle".to_string()));
}

#[tokio::test]
async fn test_compact_resume_serialization_roundtrip() {
    let mut log = EventLog::new("roundtrip-compact");
    log.log_agent_start("sess-1", Some(2));
    log.log_tool_start("tool-1", "Write");

    let compact = CompactResume::from_session(
        "sess-1",
        "task-roundtrip",
        0, // cutoff_seq
        "implement",
        2,
        true,
        "All good",
        vec!["main.rs".to_string()],
        make_test_messages(3),
        "Final review",
        &log,
        Some("Session compacted for efficiency".to_string()),
    );

    let json = serde_json::to_string(&compact).unwrap();
    let restored: CompactResume = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.boundary.session_id, "sess-1");
    assert_eq!(restored.boundary.task_id, "task-roundtrip");
    assert_eq!(restored.boundary.iteration, 2);
    assert!(restored.boundary.merge_ready);
    assert_eq!(restored.tail_events.len(), compact.tail_events.len());
}

#[tokio::test]
async fn test_compact_resume_empty_tail_after_boundary() {
    let mut log = EventLog::new("empty-tail");
    log.log_agent_start("sess-1", Some(1));

    // Capture at current point (seq 1), don't add more events
    let cutoff = log.current_seq();

    let compact = CompactResume::from_session(
        "sess-1",
        "task-1",
        cutoff, // cutoff_seq at current point
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

    assert!(compact.tail_events.is_empty());
    assert_eq!(compact.replay_events().len(), 0);
}

#[tokio::test]
async fn test_compact_resume_recent_tail_events() {
    let mut log = EventLog::new("recent-test");
    for i in 0..20 {
        log.log(
            format!("event-{}", i),
            EventCategory::System,
            EventData::empty(),
        );
    }

    // No boundary established yet, all events in tail
    let compact = CompactResume::from_session(
        "sess-1",
        "task-1",
        0, // cutoff_seq: boundary at start, all events in tail
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

    // Should have 19 events as tail (event-0 has seq=0 which is NOT > cutoff_seq=0)
    assert_eq!(compact.tail_events.len(), 19);

    // But we can still get recent ones
    let recent = compact.recent_tail_events(5);
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0].name, "event-15");
    assert_eq!(recent[4].name, "event-19");
}
