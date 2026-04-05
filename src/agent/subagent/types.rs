//! Sub-agent types and data structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    Running,
    Completed,
    Ended,
    Failed,
    Cancelled,
}

/// Callback type for inline agent streaming output (Arc for cloneability)
pub type InlineCallback = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct SubAgentHandle {
    pub id: String,
    pub task: String,
    pub status: SubAgentStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub iterations: u32,
    pub last_activity: DateTime<Utc>,
    pub error: Option<String>,
    pub result: Option<String>,
    pub parent_session_id: Option<String>,
    pub worktree_path: Option<String>,
    pub current_action: Option<String>,
}
