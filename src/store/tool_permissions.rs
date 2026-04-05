//! Tool Permission Cache Store
//!
//! Persists context-aware tool approvals to SQLite so they survive restarts.

use rusqlite::{params, Connection};
use tracing::debug;

use crate::pipeline::tool_permissions::CachedApproval;

/// Persistent store for context-aware tool approval cache.
pub struct ToolPermissionStore<'a> {
    conn: &'a Connection,
}

impl<'a> ToolPermissionStore<'a> {
    pub fn from_connection(conn: &'a Connection) -> Self {
        // Ensure table exists (created by migration, but safe for in-memory)
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                resource TEXT NOT NULL DEFAULT '',
                is_directory INTEGER NOT NULL DEFAULT 0,
                risk_level TEXT NOT NULL,
                approved_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                project_path TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        );
        // Index for fast lookups by tool+resource
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_perm_tool_resource ON tool_permissions(tool_name, resource)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_perm_expires ON tool_permissions(expires_at)",
            [],
        );
        Self { conn }
    }

    /// Save a new approval to the persistent cache.
    pub fn save(&self, approval: &CachedApproval) -> Result<(), rusqlite::Error> {
        // Delete any existing entry for same tool+resource+directory
        self.conn.execute(
            "DELETE FROM tool_permissions WHERE tool_name = ?1 AND resource = ?2 AND is_directory = ?3",
            params![approval.tool_name, approval.resource, approval.is_directory as i64],
        )?;

        self.conn.execute(
            "INSERT INTO tool_permissions (tool_name, resource, is_directory, risk_level, approved_at, expires_at, project_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                approval.tool_name,
                approval.resource,
                approval.is_directory as i64,
                approval.risk_level,
                approval.approved_at.to_rfc3339(),
                approval.expires_at.to_rfc3339(),
                approval.project_path,
            ],
        )?;

        Ok(())
    }

    /// Load all non-expired approvals from the database.
    pub fn load_active(&self) -> Result<Vec<CachedApproval>, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT tool_name, resource, is_directory, risk_level, approved_at, expires_at, project_path
             FROM tool_permissions
             WHERE expires_at > ?1
             ORDER BY approved_at DESC",
        )?;

        let rows = stmt.query_map(params![now], |row| {
            Ok(CachedApproval {
                tool_name: row.get(0)?,
                resource: row.get(1)?,
                is_directory: row.get::<_, i64>(2)? != 0,
                risk_level: row.get(3)?,
                approved_at: row
                    .get::<_, String>(4)?
                    .parse()
                    .unwrap_or(chrono::Utc::now()),
                expires_at: row
                    .get::<_, String>(5)?
                    .parse()
                    .unwrap_or(chrono::Utc::now()),
                project_path: row.get(6)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        debug!(
            count = entries.len(),
            "Loaded active tool permissions from cache"
        );
        Ok(entries)
    }

    /// Delete expired entries to keep the table clean.
    pub fn cleanup_expired(&self) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "DELETE FROM tool_permissions WHERE expires_at <= ?1",
            params![now],
        )?;
        if count > 0 {
            debug!(count, "Cleaned up expired tool permission entries");
        }
        Ok(count)
    }

    /// Clear all cached approvals (e.g., on user request).
    pub fn clear_all(&self) -> Result<usize, rusqlite::Error> {
        self.conn.execute("DELETE FROM tool_permissions", [])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_save_and_load() {
        let conn = Connection::open_in_memory().unwrap();
        let store = ToolPermissionStore::from_connection(&conn);

        let approval = CachedApproval {
            tool_name: "Write".to_string(),
            resource: "src/main.rs".to_string(),
            is_directory: false,
            risk_level: "Medium".to_string(),
            approved_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
            project_path: None,
        };

        store.save(&approval).unwrap();
        let entries = store.load_active().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "Write");
        assert_eq!(entries[0].resource, "src/main.rs");
    }

    #[test]
    fn test_expired_entries_not_loaded() {
        let conn = Connection::open_in_memory().unwrap();
        let store = ToolPermissionStore::from_connection(&conn);

        // Insert an already-expired entry directly
        conn.execute(
            "INSERT INTO tool_permissions (tool_name, resource, is_directory, risk_level, approved_at, expires_at)
             VALUES ('Write', 'old.rs', 0, 'Low', datetime('now', '-1 hour'), datetime('now', '-30 minutes'))",
            [],
        ).unwrap();

        let entries = store.load_active().unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_cleanup_expired() {
        let conn = Connection::open_in_memory().unwrap();
        let store = ToolPermissionStore::from_connection(&conn);

        // Insert active entry
        store
            .save(&CachedApproval {
                tool_name: "Write".to_string(),
                resource: "active.rs".to_string(),
                is_directory: false,
                risk_level: "Medium".to_string(),
                approved_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
                project_path: None,
            })
            .unwrap();

        // Insert expired entry
        conn.execute(
            "INSERT INTO tool_permissions (tool_name, resource, is_directory, risk_level, approved_at, expires_at)
             VALUES ('Bash', 'rm', 0, 'High', datetime('now', '-1 hour'), datetime('now', '-30 minutes'))",
            [],
        ).unwrap();

        let cleaned = store.cleanup_expired().unwrap();
        assert_eq!(cleaned, 1);

        let entries = store.load_active().unwrap();
        assert_eq!(entries.len(), 1);
    }
}
