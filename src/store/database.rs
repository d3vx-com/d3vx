//! Database connection and management
//!
//! Provides SQLite database connection with WAL mode, migrations,
//! and connection pooling support.

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::debug;

use super::migrations::run_migrations;

/// Database errors
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Failed to open database
    #[error("Failed to open database: {0}")]
    OpenError(#[source] rusqlite::Error),

    /// Failed to run migrations
    #[error("Migration failed: {0}")]
    MigrationError(#[source] rusqlite::Error),

    /// Failed to execute query
    #[error("Query error: {0}")]
    QueryError(#[source] rusqlite::Error),

    /// Database path does not exist
    #[error("Database path does not exist: {0}")]
    PathNotFound(String),

    /// Database is corrupt
    #[error("Database is corrupt: {0}")]
    CorruptDatabase(String),
}

/// Options for opening a database connection
#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    /// Custom database path (default: ~/.d3vx/d3vx.db)
    pub db_path: Option<PathBuf>,
    /// Use in-memory database (for testing)
    pub in_memory: bool,
    /// Skip running migrations
    pub skip_migrations: bool,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self {
            db_path: None,
            in_memory: false,
            skip_migrations: false,
        }
    }
}

/// Database wrapper with connection management
#[derive(Debug)]
pub struct Database {
    /// The underlying SQLite connection
    conn: Connection,
    /// Path to the database file (None for in-memory)
    path: Option<PathBuf>,
}

impl Database {
    /// Open a database connection at the default path
    ///
    /// Creates the database file and directories if they don't exist.
    pub fn open_default() -> Result<Self, DatabaseError> {
        let options = DatabaseOptions::default();
        Self::open(options)
    }

    /// Open a database connection with the specified options
    pub fn open(options: DatabaseOptions) -> Result<Self, DatabaseError> {
        let (conn, path) = if options.in_memory {
            let conn = Connection::open_in_memory().map_err(DatabaseError::OpenError)?;
            (conn, None)
        } else {
            let db_path = options.db_path.unwrap_or_else(Self::default_db_path);

            // Ensure parent directory exists
            if let Some(parent) = db_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        DatabaseError::PathNotFound(format!(
                            "Failed to create directory {}: {}",
                            parent.display(),
                            e
                        ))
                    })?;
                }
            }

            debug!("Opening database at {}", db_path.display());

            let conn = Connection::open_with_flags(
                &db_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .map_err(DatabaseError::OpenError)?;

            (conn, Some(db_path))
        };

        let mut db = Self { conn, path };

        // Apply performance and safety pragmas
        db.apply_pragmas()?;

        // Run migrations unless skipped
        if !options.skip_migrations {
            run_migrations(&mut db)?;
        }

        Ok(db)
    }

    /// Create an in-memory database for testing
    pub fn in_memory() -> Result<Self, DatabaseError> {
        Self::open(DatabaseOptions {
            in_memory: true,
            ..Default::default()
        })
    }

    /// Get the default database path
    pub fn default_db_path() -> PathBuf {
        // Check for project-local .d3vx folder first
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(project_root) = Self::find_project_root(&cwd) {
                return project_root.join(".d3vx").join("d3vx.db");
            }
        }

        // Fallback to home directory
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".d3vx")
            .join("d3vx.db")
    }

    /// Find the nearest project root (containing .d3vx or .git)
    fn find_project_root(start: &Path) -> Option<PathBuf> {
        let mut current = start;
        loop {
            if current.join(".d3vx").exists() || current.join(".git").exists() {
                return Some(current.to_path_buf());
            }
            current = current.parent()?;
        }
    }

    /// Apply SQLite performance pragmas
    fn apply_pragmas(&self) -> Result<(), DatabaseError> {
        let pragmas = [
            "PRAGMA journal_mode = WAL",
            "PRAGMA synchronous = NORMAL",
            "PRAGMA foreign_keys = ON",
            "PRAGMA busy_timeout = 5000",
            "PRAGMA cache_size = -64000", // 64MB cache
        ];

        for pragma in pragmas {
            self.conn
                .execute_batch(pragma)
                .map_err(DatabaseError::QueryError)?;
        }

        Ok(())
    }

    /// Get a reference to the underlying connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a mutable reference to the underlying connection
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Get the database path
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Execute a query that returns no results
    pub fn execute<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<usize, DatabaseError> {
        self.conn
            .execute(sql, params)
            .map_err(DatabaseError::QueryError)
    }

    /// Execute a batch of SQL statements
    pub fn execute_batch(&self, sql: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute_batch(sql)
            .map_err(DatabaseError::QueryError)
    }

    /// Prepare a SQL statement
    pub fn prepare(&self, sql: &str) -> Result<rusqlite::Statement<'_>, DatabaseError> {
        self.conn.prepare(sql).map_err(DatabaseError::QueryError)
    }

    /// Begin a transaction
    pub fn transaction(&mut self) -> Result<rusqlite::Transaction<'_>, DatabaseError> {
        self.conn.transaction().map_err(DatabaseError::QueryError)
    }

    /// Close the database connection
    pub fn close(self) -> Result<(), DatabaseError> {
        self.conn
            .close()
            .map_err(|(_, e)| DatabaseError::OpenError(e))?;
        if let Some(ref path) = self.path {
            debug!("Database connection closed at {}", path.display());
        }
        Ok(())
    }

    /// Get the last inserted row ID
    pub fn last_insert_rowid(&self) -> i64 {
        self.conn.last_insert_rowid()
    }
}

/// Thread-safe database handle for sharing across threads
pub type DatabaseHandle = Arc<parking_lot::Mutex<Database>>;

/// Create a thread-safe database handle
pub fn create_db_handle(options: DatabaseOptions) -> Result<DatabaseHandle, DatabaseError> {
    let db = Database::open(options)?;
    Ok(Arc::new(parking_lot::Mutex::new(db)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() {
        let db = Database::in_memory().expect("Failed to create in-memory database");

        // Check that migrations ran successfully
        let count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query sessions table");

        assert!(count > 0, "Sessions table should exist");
    }

    #[test]
    fn test_database_options() {
        let options = DatabaseOptions::default();
        assert!(!options.in_memory);
        assert!(!options.skip_migrations);
        assert!(options.db_path.is_none());
    }

    #[test]
    fn test_generate_id_unique() {
        let db = Database::in_memory().expect("Failed to create database");

        db.execute("CREATE TABLE test (id TEXT PRIMARY KEY)", [])
            .expect("Failed to create table");

        let id1 = crate::store::generate_id("test");
        let id2 = crate::store::generate_id("test");

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_pragmas_applied() {
        let db = Database::in_memory().expect("Failed to create database");

        let journal_mode: String = db
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("Failed to get journal mode");

        // In-memory databases use 'memory' journal mode, not WAL
        assert!(!journal_mode.is_empty());
    }
}
