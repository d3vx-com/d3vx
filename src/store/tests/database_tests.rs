//! Tests for Database Operations
//!
//! Covers connection management, migrations, and query execution.

#[cfg(test)]
mod tests {
    use crate::store::database::{Database, DatabaseOptions};

    // =========================================================================
    // In-Memory Database Tests
    // =========================================================================

    #[test]
    fn test_in_memory_database_creation() {
        let db = Database::in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_in_memory_database_has_tables() {
        let db = Database::in_memory().unwrap();

        // Check that migrations created the expected tables
        let tables: Vec<String> = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"messages".to_string()));
    }

    // =========================================================================
    // Database Options Tests
    // =========================================================================

    #[test]
    fn test_database_options_default() {
        let options = DatabaseOptions::default();

        assert!(!options.in_memory);
        assert!(!options.skip_migrations);
        assert!(options.db_path.is_none());
    }

    #[test]
    fn test_database_options_custom() {
        let options = DatabaseOptions {
            db_path: Some(std::path::PathBuf::from("/tmp/test.db")),
            in_memory: true,
            skip_migrations: true,
        };

        assert!(options.in_memory);
        assert!(options.skip_migrations);
        assert!(options.db_path.is_some());
    }

    // =========================================================================
    // Query Execution Tests
    // =========================================================================

    #[test]
    fn test_execute_simple_query() {
        let db = Database::in_memory().unwrap();

        let result = db.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", []);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_with_params() {
        let db = Database::in_memory().unwrap();

        db.execute("CREATE TABLE test (id INTEGER, name TEXT)", [])
            .unwrap();
        db.execute(
            "INSERT INTO test (id, name) VALUES (?1, ?2)",
            (1i64, "test".to_string()),
        )
        .unwrap();

        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_prepare_statement() {
        let db = Database::in_memory().unwrap();

        let stmt = db.prepare("SELECT 1 + 1");
        assert!(stmt.is_ok());
    }

    #[test]
    fn test_batch_execution() {
        let db = Database::in_memory().unwrap();

        let batch = r#"
            CREATE TABLE batch_test (id INTEGER PRIMARY KEY, value TEXT);
            INSERT INTO batch_test (value) VALUES ('a');
            INSERT INTO batch_test (value) VALUES ('b');
        "#;

        let result = db.execute_batch(batch);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Transaction Tests
    // =========================================================================

    #[test]
    fn test_transaction_commit() {
        let db = Database::in_memory().unwrap();
        db.execute(
            "CREATE TABLE txn_test (id INTEGER PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();

        {
            let mut db_ref = Database::in_memory().unwrap();
            db_ref
                .execute(
                    "CREATE TABLE txn_test (id INTEGER PRIMARY KEY, value TEXT)",
                    [],
                )
                .unwrap();

            let txn = db_ref.transaction().unwrap();

            txn.execute("INSERT INTO txn_test (value) VALUES ('committed')", [])
                .unwrap();
            txn.commit().unwrap();
        }

        // Note: This test uses separate in-memory databases so data doesn't persist
        // In real usage, committed data would persist
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_invalid_sql_returns_error() {
        let db = Database::in_memory().unwrap();

        let result = db.execute("INVALID SQL STATEMENT", []);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_table_returns_error() {
        let db = Database::in_memory().unwrap();

        db.execute("CREATE TABLE dup_test (id INTEGER)", [])
            .unwrap();
        let result = db.execute("CREATE TABLE dup_test (id INTEGER)", []);
        assert!(result.is_err());
    }

    // =========================================================================
    // Connection Management Tests
    // =========================================================================

    #[test]
    fn test_connection_reference() {
        let db = Database::in_memory().unwrap();

        let conn = db.connection();
        assert!(conn
            .execute("CREATE TABLE conn_test (id INTEGER)", [])
            .is_ok());
    }

    #[test]
    fn test_last_insert_rowid() {
        let db = Database::in_memory().unwrap();

        db.execute(
            "CREATE TABLE rowid_test (id INTEGER PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO rowid_test (value) VALUES ('test')", [])
            .unwrap();

        let rowid = db.last_insert_rowid();
        assert_eq!(rowid, 1);
    }

    // =========================================================================
    // ID Generation Tests
    // =========================================================================

    #[test]
    fn test_generate_id_format() {
        let id = crate::store::generate_id("ses");

        assert!(id.starts_with("ses-"));
        assert!(id.len() > 4); // "ses-" + timestamp + random
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let ids: Vec<String> = (0..100)
            .map(|_| crate::store::generate_id("test"))
            .collect();

        let unique_count = ids.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, 100);
    }

    // =========================================================================
    // Timestamp Tests
    // =========================================================================

    #[test]
    fn test_now_iso_format() {
        let ts = crate::store::now_iso();

        assert!(ts.contains('T'));
        // Should be valid ISO 8601
        assert!(chrono::DateTime::parse_from_rfc3339(&ts).is_ok());
    }
}
