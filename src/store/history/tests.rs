//! Tests for the history reader module.

use super::*;
use crate::store::database::Database;
use crate::store::event::EventStore;
use crate::store::message::{MessageRecord, MessageRole, MessageStore, NewMessage};
use crate::store::session::{NewSession, SessionStore};

fn create_test_db() -> Database {
    Database::in_memory().expect("Failed to create in-memory database")
}

fn create_test_session<'a>(db: &'a Database, id: &str) -> SessionStore<'a> {
    let store = SessionStore::new(db);
    store
        .create(NewSession {
            id: Some(id.to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test-model".to_string(),
            messages: None,
            token_count: Some(100),
            summary: None,
            project_path: Some("/test/project".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");
    store
}

#[test]
fn test_history_bounds_defaults() {
    let bounds = HistoryBounds::default();
    assert_eq!(bounds.limit, 0);
    assert_eq!(bounds.offset, 0);
}

#[test]
fn test_history_bounds_new() {
    let bounds = HistoryBounds::new(25, 10);
    assert_eq!(bounds.limit, 25);
    assert_eq!(bounds.offset, 10);
}

#[test]
fn test_history_bounds_limit() {
    let bounds = HistoryBounds::limit(50);
    assert_eq!(bounds.limit, 50);
    assert_eq!(bounds.offset, 0);
}

#[test]
fn test_history_bounds_page() {
    let bounds = HistoryBounds::page(2, 20);
    assert_eq!(bounds.limit, 20);
    assert_eq!(bounds.offset, 40);
}

#[test]
fn test_history_bounds_sql_limit_capped() {
    let bounds = HistoryBounds::new(5000, 0);
    assert_eq!(bounds.sql_limit(), 1000);
}

#[test]
fn test_history_filter_defaults() {
    let filter = HistoryFilter::default();
    assert!(filter.project_path.is_none());
    assert!(filter.task_id.is_none());
    assert!(filter.session_state.is_none());
    assert!(filter.from_time.is_none());
    assert!(filter.to_time.is_none());
}

#[test]
fn test_history_filter_chain() {
    let filter = HistoryFilter::new()
        .with_project("/path/to/project")
        .with_task("task-123")
        .with_state("RUNNING");

    assert_eq!(filter.project_path.as_deref(), Some("/path/to/project"));
    assert_eq!(filter.task_id.as_deref(), Some("task-123"));
    assert_eq!(filter.session_state.as_deref(), Some("RUNNING"));
}

#[test]
fn test_history_filter_time_range() {
    let from = chrono::Utc::now() - chrono::Duration::days(7);
    let to = chrono::Utc::now();

    let filter = HistoryFilter::new().with_time_range(Some(from), Some(to));

    assert!(filter.from_time.is_some());
    assert!(filter.to_time.is_some());
}

#[test]
fn test_history_query_defaults() {
    let query = HistoryQuery::default();
    assert_eq!(query.kind, HistoryKind::Sessions);
    assert_eq!(query.bounds.limit, 50);
}

#[test]
fn test_history_query_factory_methods() {
    let sessions = HistoryQuery::sessions();
    assert_eq!(sessions.kind, HistoryKind::Sessions);

    let events = HistoryQuery::events();
    assert_eq!(events.kind, HistoryKind::Events);

    let recent = HistoryQuery::recent_sessions(10);
    assert_eq!(recent.kind, HistoryKind::Sessions);
    assert_eq!(recent.bounds.limit, 10);

    let recent_events = HistoryQuery::recent_events(20);
    assert_eq!(recent_events.kind, HistoryKind::Events);
    assert_eq!(recent_events.bounds.limit, 20);
}

#[test]
fn test_history_query_builder() {
    let query = HistoryQuery::sessions()
        .filter(HistoryFilter::new().with_project("/test"))
        .bounds(HistoryBounds::limit(25));

    assert_eq!(query.kind, HistoryKind::Sessions);
    assert_eq!(query.bounds.limit, 25);
    assert_eq!(query.filter.project_path.as_deref(), Some("/test"));
}

#[test]
fn test_history_kind_display() {
    assert_eq!(HistoryKind::Sessions.to_string(), "sessions");
    assert_eq!(HistoryKind::Events.to_string(), "events");
    assert_eq!(HistoryKind::All.to_string(), "all");
}

#[test]
fn test_transcript_role_from_str() {
    assert_eq!(TranscriptRole::from("user"), TranscriptRole::User);
    assert_eq!(TranscriptRole::from("assistant"), TranscriptRole::Assistant);
    assert_eq!(TranscriptRole::from("system"), TranscriptRole::System);
    assert_eq!(TranscriptRole::from("unknown"), TranscriptRole::System);
}

#[test]
fn test_transcript_entry_preview() {
    let entry = TranscriptEntry {
        index: 0,
        role: TranscriptRole::User,
        content: "This is a short message".to_string(),
        timestamp: chrono::Utc::now(),
        token_count: None,
    };

    assert_eq!(entry.preview(100), "This is a short message");
    assert_eq!(entry.preview(10), "This is a...");

    assert!(entry.is_user());
    assert!(!entry.is_assistant());
    assert!(!entry.is_system());
}

#[test]
fn test_transcript_summary_from_entries() {
    let entries = vec![
        TranscriptEntry {
            index: 0,
            role: TranscriptRole::User,
            content: "Hello".to_string(),
            timestamp: chrono::Utc::now(),
            token_count: Some(5),
        },
        TranscriptEntry {
            index: 1,
            role: TranscriptRole::Assistant,
            content: "Hi there!".to_string(),
            timestamp: chrono::Utc::now(),
            token_count: Some(10),
        },
        TranscriptEntry {
            index: 2,
            role: TranscriptRole::User,
            content: "How are you?".to_string(),
            timestamp: chrono::Utc::now(),
            token_count: Some(7),
        },
    ];

    let summary = TranscriptSummary::from_entries(&entries);

    assert_eq!(summary.total_entries, 3);
    assert_eq!(summary.user_messages, 2);
    assert_eq!(summary.assistant_messages, 1);
    assert_eq!(summary.system_messages, 0);
    assert_eq!(summary.total_tokens, Some(22));
}

#[test]
fn test_transcript_summary_empty() {
    let summary = TranscriptSummary::default();
    assert_eq!(summary.total_entries, 0);
    assert_eq!(summary.duration(), None);
}

#[test]
fn test_history_reader_recent_sessions() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "session-1");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let bounds = HistoryBounds::limit(10);
    let filter = HistoryFilter::new();

    let sessions = reader
        .get_recent_sessions(&bounds, &filter)
        .expect("Failed to get sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "session-1");
}

#[test]
fn test_history_reader_pagination() {
    let db = create_test_db();
    let session_store = SessionStore::new(&db);

    for i in 0..10 {
        session_store
            .create(NewSession {
                id: Some(format!("session-{}", i)),
                task_id: None,
                provider: "test".to_string(),
                model: "test".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: None,
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .expect("Failed to create session");
    }

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let page1 = reader
        .get_recent_sessions(&HistoryBounds::page(0, 3), &HistoryFilter::new())
        .expect("Failed to get page 1");
    assert_eq!(page1.len(), 3);

    let page2 = reader
        .get_recent_sessions(&HistoryBounds::page(1, 3), &HistoryFilter::new())
        .expect("Failed to get page 2");
    assert_eq!(page2.len(), 3);

    let page3 = reader
        .get_recent_sessions(&HistoryBounds::page(2, 3), &HistoryFilter::new())
        .expect("Failed to get page 3");
    assert_eq!(page3.len(), 3);

    let page4 = reader
        .get_recent_sessions(&HistoryBounds::page(3, 3), &HistoryFilter::new())
        .expect("Failed to get page 4");
    assert_eq!(page4.len(), 1);
}

#[test]
fn test_history_reader_filter_by_project() {
    let db = create_test_db();
    let session_store = SessionStore::new(&db);

    session_store
        .create(NewSession {
            id: Some("sess-A".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/project/a".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    session_store
        .create(NewSession {
            id: Some("sess-B".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/project/b".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let sessions = reader
        .get_recent_sessions(
            &HistoryBounds::limit(100),
            &HistoryFilter::new().with_project("/project/a"),
        )
        .expect("Failed to get sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess-A");
}

#[test]
fn test_history_reader_get_session() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "test-session");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let found = reader
        .get_session("test-session")
        .expect("Failed to get session");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "test-session");

    let not_found = reader
        .get_session("non-existent")
        .expect("Failed to get session");
    assert!(not_found.is_none());
}

#[test]
fn test_history_reader_latest_session() {
    let db = create_test_db();
    let session_store = SessionStore::new(&db);

    session_store
        .create(NewSession {
            id: Some("old".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/test".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    std::thread::sleep(std::time::Duration::from_millis(10));

    session_store
        .create(NewSession {
            id: Some("new".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/test".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let latest = reader
        .get_latest_session(Some("/test"))
        .expect("Failed to get latest");
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().id, "new");
}

#[test]
fn test_history_reader_stats() {
    let db = create_test_db();
    let session_store = SessionStore::new(&db);

    session_store
        .create(NewSession {
            id: Some("s1".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/project".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    session_store
        .create(NewSession {
            id: Some("s2".to_string()),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: None,
            token_count: None,
            summary: None,
            project_path: Some("/project".to_string()),
            parent_session_id: None,
            metadata: None,
            state: None,
        })
        .expect("Failed to create session");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let stats = reader.get_stats().expect("Failed to get stats");
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.sessions_by_project.get("/project"), Some(&2));
    assert!(stats.oldest_session.is_some());
    assert!(stats.newest_session.is_some());
}

#[test]
fn test_history_result_counts() {
    let result = HistoryResult::Sessions(vec![]);
    assert!(result.is_empty());
    assert_eq!(result.session_count(), 0);
    assert_eq!(result.event_count(), 0);

    let sessions = vec![crate::store::Session {
        id: "test".to_string(),
        task_id: None,
        provider: "test".to_string(),
        model: "test".to_string(),
        messages: "[]".to_string(),
        token_count: 0,
        summary: None,
        project_path: None,
        parent_session_id: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        metadata: "{}".to_string(),
        state: crate::store::session::SessionState::Idle,
    }];

    let result = HistoryResult::Sessions(sessions);
    assert!(!result.is_empty());
    assert_eq!(result.session_count(), 1);
    assert_eq!(result.event_count(), 0);
}

#[test]
fn test_history_query_execute_sessions() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "exec-test");

    let message_store = MessageStore::new(&db);
    let event_store = EventStore::new(&db);
    let reader = HistoryReader::new(&db, &session_store, &message_store, &event_store);

    let query = HistoryQuery::sessions().bounds(HistoryBounds::limit(10));
    let result = reader.execute(&query).expect("Failed to execute query");

    match result {
        HistoryResult::Sessions(sessions) => {
            assert_eq!(sessions.len(), 1);
        }
        _ => panic!("Expected Sessions result"),
    }
}

#[test]
fn test_transcript_reader_entries() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "transcript-test");
    let message_store = MessageStore::new(&db);

    message_store
        .add(NewMessage {
            session_id: "transcript-test".to_string(),
            role: MessageRole::User,
            content: "Hello!".to_string(),
            content_type: None,
            token_count: Some(5),
        })
        .expect("Failed to add message");

    message_store
        .add(NewMessage {
            session_id: "transcript-test".to_string(),
            role: MessageRole::Assistant,
            content: "Hi there!".to_string(),
            content_type: None,
            token_count: Some(10),
        })
        .expect("Failed to add message");

    let reader = TranscriptReader::new(&session_store, &message_store);

    let entries = reader
        .get_entries("transcript-test", None)
        .expect("Failed to get entries");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].role, TranscriptRole::User);
    assert_eq!(entries[0].content, "Hello!");
    assert_eq!(entries[1].role, TranscriptRole::Assistant);
    assert_eq!(entries[1].content, "Hi there!");
}

#[test]
fn test_transcript_reader_pagination() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "paginated-test");
    let message_store = MessageStore::new(&db);

    for i in 0..10 {
        message_store
            .add(NewMessage {
                session_id: "paginated-test".to_string(),
                role: MessageRole::User,
                content: format!("Message {}", i),
                content_type: None,
                token_count: None,
            })
            .expect("Failed to add message");
    }

    let reader = TranscriptReader::new(&session_store, &message_store);

    let first_page = reader
        .get_entries("paginated-test", Some(&HistoryBounds::page(0, 3)))
        .expect("Failed to get first page");
    assert_eq!(first_page.len(), 3);
    assert_eq!(first_page[0].index, 0);

    let second_page = reader
        .get_entries("paginated-test", Some(&HistoryBounds::page(1, 3)))
        .expect("Failed to get second page");
    assert_eq!(second_page.len(), 3);
    assert_eq!(second_page[0].index, 3);
}

#[test]
fn test_transcript_summary() {
    let db = create_test_db();
    let session_store = create_test_session(&db, "summary-test");
    let message_store = MessageStore::new(&db);

    let roles: Vec<MessageRole> = vec![MessageRole::User, MessageRole::Assistant];
    for i in 0..6 {
        let role = roles[i % 2];
        message_store
            .add(NewMessage {
                session_id: "summary-test".to_string(),
                role,
                content: format!("Message {}", i),
                content_type: None,
                token_count: Some(5),
            })
            .expect("Failed to add message");
    }

    let reader = TranscriptReader::new(&session_store, &message_store);

    let summary = reader
        .get_summary("summary-test")
        .expect("Failed to get summary");

    assert_eq!(summary.total_entries, 6);
    assert_eq!(summary.user_messages, 3);
    assert_eq!(summary.assistant_messages, 3);
    assert_eq!(summary.total_tokens, Some(30));
    assert!(summary.started_at.is_some());
    assert!(summary.ended_at.is_some());
}

#[test]
fn test_history_bounds_last() {
    let bounds = HistoryBounds::last(5);
    assert_eq!(bounds.limit, 5);
    assert_eq!(bounds.offset, 0);
}
