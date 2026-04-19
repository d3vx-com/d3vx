//! Tests for the tool execution audit store.
//!
//! Writes must round-trip through the `tool_executions` table and
//! retrieval must preserve ordering and flag semantics. These tests
//! guard the contract consumed by the dashboard and observability
//! feeds.

#[cfg(test)]
mod tests {
    use crate::store::database::Database;
    use crate::store::session::{NewSession, SessionStore};
    use crate::store::tool_execution::{NewToolExecution, ToolExecutionStore};

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn create_test_session(db: &Database) -> String {
        let store = SessionStore::new(db);
        let session = store
            .create(NewSession {
                id: None,
                task_id: None,
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4".to_string(),
                messages: None,
                token_count: None,
                summary: None,
                project_path: None,
                parent_session_id: None,
                metadata: None,
                state: None,
            })
            .unwrap();
        session.id
    }

    fn make_exec(session_id: &str, tool: &str) -> NewToolExecution {
        NewToolExecution {
            session_id: session_id.to_string(),
            tool_name: tool.to_string(),
            tool_input: serde_json::json!({ "arg": "value" }),
            tool_result: Some("ok".to_string()),
            is_error: false,
            duration_ms: Some(42),
        }
    }

    #[test]
    fn record_inserts_row_and_returns_it() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        let rec = store.record(make_exec(&session_id, "bash")).unwrap();
        assert!(rec.id > 0);
        assert_eq!(rec.session_id, session_id);
        assert_eq!(rec.tool_name, "bash");
        assert_eq!(rec.tool_result.as_deref(), Some("ok"));
        assert!(!rec.is_error);
        assert_eq!(rec.duration_ms, Some(42));
        assert!(!rec.created_at.is_empty());
    }

    #[test]
    fn record_serialises_json_input_faithfully() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        let exec = NewToolExecution {
            session_id: session_id.clone(),
            tool_name: "Edit".to_string(),
            tool_input: serde_json::json!({
                "file_path": "/tmp/x.rs",
                "old_string": "foo",
                "new_string": "bar",
                "replace_all": false,
            }),
            tool_result: Some("1 replacement".to_string()),
            is_error: false,
            duration_ms: Some(12),
        };
        let rec = store.record(exec).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&rec.tool_input).unwrap();
        assert_eq!(parsed["file_path"], "/tmp/x.rs");
        assert_eq!(parsed["replace_all"], false);
    }

    #[test]
    fn list_for_session_returns_records_in_insertion_order() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        for tool in ["Read", "Bash", "Edit"] {
            store.record(make_exec(&session_id, tool)).unwrap();
        }

        let rows = store.list_for_session(&session_id).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].tool_name, "Read");
        assert_eq!(rows[1].tool_name, "Bash");
        assert_eq!(rows[2].tool_name, "Edit");
    }

    #[test]
    fn list_for_session_isolates_sessions() {
        let db = create_test_db();
        let s1 = create_test_session(&db);
        let s2 = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        store.record(make_exec(&s1, "Read")).unwrap();
        store.record(make_exec(&s2, "Bash")).unwrap();
        store.record(make_exec(&s1, "Edit")).unwrap();

        let s1_rows = store.list_for_session(&s1).unwrap();
        assert_eq!(s1_rows.len(), 2);
        assert_eq!(s1_rows[0].tool_name, "Read");
        assert_eq!(s1_rows[1].tool_name, "Edit");

        let s2_rows = store.list_for_session(&s2).unwrap();
        assert_eq!(s2_rows.len(), 1);
        assert_eq!(s2_rows[0].tool_name, "Bash");
    }

    #[test]
    fn is_error_flag_round_trips() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        let mut err_exec = make_exec(&session_id, "Bash");
        err_exec.is_error = true;
        err_exec.tool_result = Some("command failed".to_string());
        store.record(err_exec).unwrap();

        let rows = store.list_for_session(&session_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_error);
        assert_eq!(rows[0].tool_result.as_deref(), Some("command failed"));
    }

    #[test]
    fn optional_fields_support_none() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        let exec = NewToolExecution {
            session_id: session_id.clone(),
            tool_name: "Think".to_string(),
            tool_input: serde_json::Value::Null,
            tool_result: None,
            is_error: false,
            duration_ms: None,
        };
        let rec = store.record(exec).unwrap();
        assert!(rec.tool_result.is_none());
        assert!(rec.duration_ms.is_none());

        let round_tripped = &store.list_for_session(&session_id).unwrap()[0];
        assert!(round_tripped.tool_result.is_none());
        assert!(round_tripped.duration_ms.is_none());
    }

    #[test]
    fn count_for_session_is_accurate() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        assert_eq!(store.count_for_session(&session_id).unwrap(), 0);

        for _ in 0..5 {
            store.record(make_exec(&session_id, "Read")).unwrap();
        }
        assert_eq!(store.count_for_session(&session_id).unwrap(), 5);
    }

    #[test]
    fn list_recent_returns_newest_first_across_sessions() {
        let db = create_test_db();
        let s1 = create_test_session(&db);
        let s2 = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        store.record(make_exec(&s1, "A")).unwrap();
        store.record(make_exec(&s2, "B")).unwrap();
        store.record(make_exec(&s1, "C")).unwrap();

        let recent = store.list_recent(10).unwrap();
        assert_eq!(recent.len(), 3);
        // Newest-first: insertion order was A, B, C → recent is C, B, A
        assert_eq!(recent[0].tool_name, "C");
        assert_eq!(recent[1].tool_name, "B");
        assert_eq!(recent[2].tool_name, "A");
    }

    #[test]
    fn list_recent_respects_limit() {
        let db = create_test_db();
        let session_id = create_test_session(&db);
        let store = ToolExecutionStore::new(&db);

        for _ in 0..10 {
            store.record(make_exec(&session_id, "Read")).unwrap();
        }

        let recent = store.list_recent(3).unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn record_for_nonexistent_session_fails() {
        // The FK constraint on tool_executions.session_id enforces that
        // orphan records cannot be inserted. Documents the constraint
        // so callers know to create a session first.
        let db = create_test_db();
        let store = ToolExecutionStore::new(&db);

        let err = store
            .record(make_exec("nonexistent-session-id", "Read"))
            .unwrap_err();
        // Any DatabaseError variant is acceptable here — the contract
        // is "insert fails," not a specific error shape.
        let _ = err;
    }
}
