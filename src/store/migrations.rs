//! Database migrations
//!
//! SQL schema for all tables with version tracking. Migrations are run
//! automatically when the database is opened.

use tracing::{debug, info};

use super::database::DatabaseError;

/// Current schema version
const SCHEMA_VERSION: i32 = 103;

/// Run all migrations on the database
pub fn run_migrations(db: &mut super::database::Database) -> Result<(), DatabaseError> {
    let conn = db.connection_mut();

    // Create migrations tracking table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )
    .map_err(DatabaseError::MigrationError)?;

    // Check current version
    let current_version: i32 = conn
        .query_row("SELECT COALESCE(MAX(id), 0) FROM _migrations", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    if current_version >= SCHEMA_VERSION {
        debug!(
            "Database schema is up to date (version {})",
            current_version
        );
        return Ok(());
    }

    // Run migrations in order
    let migrations = get_migrations();

    for (version, name, sql) in migrations {
        if version <= current_version {
            continue;
        }

        debug!("Running migration: {} (version {})", name, version);

        conn.execute_batch(sql)
            .map_err(DatabaseError::MigrationError)?;

        conn.execute(
            "INSERT INTO _migrations (id, name) VALUES (?1, ?2)",
            rusqlite::params![version, name],
        )
        .map_err(DatabaseError::MigrationError)?;

        info!("Migration applied: {}", name);
    }

    Ok(())
}

/// Get all migrations as (version, name, sql) tuples
fn get_migrations() -> Vec<(i32, &'static str, &'static str)> {
    vec![
        (100, "100_initial_schema_consolidation", SCHEMA_V1),
        (101, "101_add_session_metadata", SCHEMA_V2),
        (102, "102_task_runs_workspaces_workers", SCHEMA_V3),
        (103, "103_add_session_state", SCHEMA_V4),
    ]
}

/// Schema version 1: Initial tables
const SCHEMA_V1: &str = r#"
-- ============================================================================
-- Sessions table
-- Stores conversation history for the REPL and pipeline phases
-- ============================================================================
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    task_id TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    messages TEXT NOT NULL DEFAULT '[]',
    token_count INTEGER DEFAULT 0,
    summary TEXT,
    project_path TEXT,
    parent_session_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sessions_task ON sessions(task_id);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at);

-- ============================================================================
-- Tasks table
-- Kanban state machine for task management
-- ============================================================================
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL DEFAULT 'BACKLOG',
    priority INTEGER DEFAULT 0,
    batch_id TEXT,
    worktree_path TEXT,
    worktree_branch TEXT,
    pipeline_phase TEXT,
    checkpoint_data TEXT,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    depends_on TEXT DEFAULT '[]',
    error TEXT,
    metadata TEXT DEFAULT '{}',
    project_path TEXT,
    agent_role TEXT,
    log_file TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_state ON tasks(state);
CREATE INDEX IF NOT EXISTS idx_tasks_batch ON tasks(batch_id);
CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project_path);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority DESC);

-- ============================================================================
-- Task logs table
-- Audit trail of every task action
-- ============================================================================
CREATE TABLE IF NOT EXISTS task_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    event TEXT NOT NULL,
    data TEXT DEFAULT '{}',
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_task_logs_task ON task_logs(task_id);
CREATE INDEX IF NOT EXISTS idx_task_logs_phase ON task_logs(phase);

-- ============================================================================
-- Messages table
-- Individual messages for each session
-- ============================================================================
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    content_type TEXT DEFAULT 'text',
    token_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at);

-- ============================================================================
-- Memory entries table
-- Hybrid: SQLite index for fast search, markdown files for content
-- ============================================================================
CREATE TABLE IF NOT EXISTS memory_entries (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL CHECK(type IN ('project', 'task', 'codebase', 'error', 'conversation')),
    title TEXT NOT NULL,
    file_path TEXT NOT NULL,
    tags TEXT DEFAULT '[]',
    summary TEXT,
    project_path TEXT,
    relevance_score REAL DEFAULT 1.0,
    access_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_entries(type);
CREATE INDEX IF NOT EXISTS idx_memory_project ON memory_entries(project_path);
CREATE INDEX IF NOT EXISTS idx_memory_relevance ON memory_entries(relevance_score DESC);

-- ============================================================================
-- Tool executions table
-- Record of tool use for debugging and auditing
-- ============================================================================
CREATE TABLE IF NOT EXISTS tool_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    tool_input TEXT NOT NULL DEFAULT '{}',
    tool_result TEXT,
    is_error INTEGER DEFAULT 0,
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tool_executions_session ON tool_executions(session_id);
CREATE INDEX IF NOT EXISTS idx_tool_executions_tool ON tool_executions(tool_name);
CREATE INDEX IF NOT EXISTS idx_tool_executions_created ON tool_executions(created_at);
"#;

// ============================================================================
// Schema version 2: Session metadata
// ============================================================================
const SCHEMA_V2: &str = r#"
ALTER TABLE sessions ADD COLUMN metadata TEXT DEFAULT '{}';
"#;

// ============================================================================
// Schema version 3: Task runs, workspaces, workers, events, dependencies
// ============================================================================
const SCHEMA_V3: &str = r#"
-- ============================================================================
-- Task Runs table
-- Tracks each execution attempt of a task
-- ============================================================================
CREATE TABLE IF NOT EXISTS task_runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK(status IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED')),
    worker_id TEXT,
    workspace_id TEXT,
    started_at TEXT,
    ended_at TEXT,
    failure_reason TEXT,
    summary TEXT,
    metrics_json TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_task_runs_task ON task_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_task_runs_status ON task_runs(status);
CREATE INDEX IF NOT EXISTS idx_task_runs_worker ON task_runs(worker_id);
CREATE INDEX IF NOT EXISTS idx_task_runs_started ON task_runs(started_at);

-- ============================================================================
-- Workspaces table
-- Isolated execution environments (worktrees, mirrors, etc.)
-- ============================================================================
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    workspace_type TEXT NOT NULL DEFAULT 'worktree' CHECK(workspace_type IN ('direct', 'worktree', 'mirror')),
    path TEXT NOT NULL,
    branch_name TEXT,
    base_ref TEXT,
    repo_root TEXT,
    task_scope_path TEXT,
    scope_mode TEXT NOT NULL DEFAULT 'REPO' CHECK(scope_mode IN ('REPO', 'SUBDIR', 'NESTED_REPO', 'MULTI_REPO')),
    status TEXT NOT NULL DEFAULT 'CREATING' CHECK(status IN ('CREATING', 'READY', 'ACTIVE', 'CLEANING', 'CLEANED', 'FAILED')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    cleaned_at TEXT,
    metadata_json TEXT DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_workspaces_task ON workspaces(task_id);
CREATE INDEX IF NOT EXISTS idx_workspaces_run ON workspaces(run_id);
CREATE INDEX IF NOT EXISTS idx_workspaces_status ON workspaces(status);
CREATE INDEX IF NOT EXISTS idx_workspaces_path ON workspaces(path);

-- ============================================================================
-- Workers table
-- Agent processes that execute tasks
-- ============================================================================
CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY,
    worker_type TEXT NOT NULL DEFAULT 'agent' CHECK(worker_type IN ('agent', 'vex', 'daemon')),
    status TEXT NOT NULL DEFAULT 'IDLE' CHECK(status IN ('IDLE', 'BUSY', 'OFFLINE', 'ERROR')),
    current_run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    last_heartbeat_at TEXT,
    capabilities_json TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
CREATE INDEX IF NOT EXISTS idx_workers_run ON workers(current_run_id);

-- ============================================================================
-- Task Events table (append-only event log)
-- Comprehensive audit trail for task lifecycle
-- ============================================================================
CREATE TABLE IF NOT EXISTS task_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    event_data TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_run ON task_events(run_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type ON task_events(event_type);
CREATE INDEX IF NOT EXISTS idx_task_events_created ON task_events(created_at);

-- ============================================================================
-- Task Dependencies table
-- Explicit dependency relationships between tasks
-- ============================================================================
CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_deps_task ON task_dependencies(task_id);
CREATE INDEX IF NOT EXISTS idx_task_deps_depends ON task_dependencies(depends_on_task_id);

-- ============================================================================
-- Update tasks table with new columns
-- ============================================================================
ALTER TABLE tasks ADD COLUMN execution_mode TEXT DEFAULT 'AUTO' CHECK(execution_mode IN ('DIRECT', 'VEX', 'AUTO'));
ALTER TABLE tasks ADD COLUMN repo_root TEXT;
ALTER TABLE tasks ADD COLUMN task_scope_path TEXT;
ALTER TABLE tasks ADD COLUMN scope_mode TEXT DEFAULT 'REPO' CHECK(scope_mode IN ('REPO', 'SUBDIR', 'NESTED_REPO', 'MULTI_REPO'));
ALTER TABLE tasks ADD COLUMN parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL;
"#;

// ============================================================================
// Schema version 4: Agent 15-State Machine Session States
// ============================================================================
const SCHEMA_V4: &str = r#"
ALTER TABLE sessions ADD COLUMN state TEXT NOT NULL DEFAULT 'SPAWNING';
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_successfully() {
        let db = crate::store::database::Database::in_memory()
            .expect("Failed to create in-memory database");

        // Migrations should have run during creation
        let conn = db.connection();

        // Check that all tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect()
            })
            .expect("Failed to query tables");

        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"task_logs".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"memory_entries".to_string()));
        assert!(tables.contains(&"tool_executions".to_string()));
        assert!(tables.contains(&"_migrations".to_string()));
    }

    #[test]
    fn test_migration_version_tracking() {
        let db = crate::store::database::Database::in_memory()
            .expect("Failed to create in-memory database");

        let conn = db.connection();

        let version: i32 = conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM _migrations", [], |row| {
                row.get(0)
            })
            .expect("Failed to get migration version");

        assert!(version >= SCHEMA_VERSION);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        // Create database with migrations
        let mut db1 =
            crate::store::database::Database::in_memory().expect("Failed to create first database");

        // Run migrations again (should be no-op)
        run_migrations(&mut db1).expect("Re-running migrations should succeed");

        // Check version is still the same
        let version: i32 = db1
            .connection()
            .query_row("SELECT COALESCE(MAX(id), 0) FROM _migrations", [], |row| {
                row.get(0)
            })
            .expect("Failed to get migration version");

        assert_eq!(version, SCHEMA_VERSION);
    }
}
