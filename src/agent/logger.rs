//! JSONL Logger for Agent Events
//!
//! Writes agent events to a session file for persistence and deep-dive exploration.

use crate::agent::agent_loop::AgentEvent;
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub struct JsonlLogger {
    path: PathBuf,
}

impl JsonlLogger {
    pub fn new(session_id: &str) -> Self {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".d3vx");
        path.push("sessions");
        let _ = std::fs::create_dir_all(&path);
        path.push(format!("{}.jsonl", session_id));
        Self { path }
    }

    pub fn log(&self, event: &AgentEvent) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }
}
