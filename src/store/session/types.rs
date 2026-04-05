//! Session types and data structures
//!
//! Defines the session lifecycle states and data structs.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The explicit 15 granular lifecycle states of an Agent tracking session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    Spawning,
    Initializing,
    Running,
    Idle,
    WaitingInput,
    Blocked,
    Stopping,
    Stopped,
    Crashed,
    Failed,
    Merging,
    Merged,
    Abandoned,
    Cleaning,
    Cleaned,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::Spawning
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Spawning => "SPAWNING",
            Self::Initializing => "INITIALIZING",
            Self::Running => "RUNNING",
            Self::Idle => "IDLE",
            Self::WaitingInput => "WAITING_INPUT",
            Self::Blocked => "BLOCKED",
            Self::Stopping => "STOPPING",
            Self::Stopped => "STOPPED",
            Self::Crashed => "CRASHED",
            Self::Failed => "FAILED",
            Self::Merging => "MERGING",
            Self::Merged => "MERGED",
            Self::Abandoned => "ABANDONED",
            Self::Cleaning => "CLEANING",
            Self::Cleaned => "CLEANED",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for SessionState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SPAWNING" => Ok(Self::Spawning),
            "INITIALIZING" => Ok(Self::Initializing),
            "RUNNING" => Ok(Self::Running),
            "IDLE" => Ok(Self::Idle),
            "WAITING_INPUT" => Ok(Self::WaitingInput),
            "BLOCKED" => Ok(Self::Blocked),
            "STOPPING" => Ok(Self::Stopping),
            "STOPPED" => Ok(Self::Stopped),
            "CRASHED" => Ok(Self::Crashed),
            "FAILED" => Ok(Self::Failed),
            "MERGING" => Ok(Self::Merging),
            "MERGED" => Ok(Self::Merged),
            "ABANDONED" => Ok(Self::Abandoned),
            "CLEANING" => Ok(Self::Cleaning),
            "CLEANED" => Ok(Self::Cleaned),
            _ => Err(format!("Unknown SessionState: {}", s)),
        }
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Associated task ID (if any)
    pub task_id: Option<String>,
    /// LLM provider name
    pub provider: String,
    /// Model identifier
    pub model: String,
    /// Serialized messages (JSON array)
    pub messages: String,
    /// Total token count
    pub token_count: i64,
    /// Session summary (optional)
    pub summary: Option<String>,
    /// Project path (optional)
    pub project_path: Option<String>,
    /// Parent session ID for hierarchical sessions
    pub parent_session_id: Option<String>,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Last update timestamp (ISO 8601)
    pub updated_at: String,
    /// Custom metadata (JSON object)
    pub metadata: String,
    /// Current granular session state
    pub state: SessionState,
}

/// Input for creating a new session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSession {
    /// Optional custom ID (auto-generated if not provided)
    pub id: Option<String>,
    /// Associated task ID
    pub task_id: Option<String>,
    /// LLM provider name
    pub provider: String,
    /// Model identifier
    pub model: String,
    /// Initial messages (JSON array)
    pub messages: Option<String>,
    /// Initial token count
    pub token_count: Option<i64>,
    /// Session summary
    pub summary: Option<String>,
    /// Project path
    pub project_path: Option<String>,
    /// Parent session ID
    pub parent_session_id: Option<String>,
    /// Custom metadata (JSON object)
    pub metadata: Option<String>,
    /// Explicit initial state hook
    pub state: Option<SessionState>,
}

/// Fields to update on a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionUpdate {
    /// Updated messages
    pub messages: Option<String>,
    /// Updated token count
    pub token_count: Option<i64>,
    /// Updated summary
    pub summary: Option<String>,
    /// Updated metadata
    pub metadata: Option<String>,
    /// Optional transition state mutation
    pub state: Option<SessionState>,
}

/// Options for listing sessions
#[derive(Debug, Clone, Default)]
pub struct SessionListOptions {
    /// Filter by project path
    pub project_path: Option<String>,
    /// Filter by task ID
    pub task_id: Option<String>,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}
