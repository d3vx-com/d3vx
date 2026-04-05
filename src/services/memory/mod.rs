use anyhow::Result;
use rusqlite::{params, Connection};

pub struct MemoryItem {
    pub name: String,
    pub content: String,
}

pub struct MemorySearch {
    conn: Connection,
}

impl MemorySearch {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Initialize FTS5 table
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_index USING fts5(name, content)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn index(&self, name: &str, content: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO memory_index(name, content) VALUES (?, ?)",
            params![name, content],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<MemoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, content FROM memory_index WHERE memory_index MATCH ? ORDER BY rank",
        )?;

        let rows = stmt.query_map(params![query], |row| {
            Ok(MemoryItem {
                name: row.get(0)?,
                content: row.get(1)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }
}
