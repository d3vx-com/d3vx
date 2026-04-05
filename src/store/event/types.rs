//! Event types and data structures
//!
//! Defines the event type enum and data structs for the append-only event log.

use serde::{Deserialize, Serialize};

/// Types of task events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    // Task lifecycle
    TaskCreated,
    TaskTriaged,
    TaskQueued,
    TaskScheduled,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskCancelled,
    TaskRetried,

    // State transitions
    StateChanged,

    // Run events
    RunStarted,
    RunCompleted,
    RunFailed,
    WorkerAssigned,
    WorkerReleased,

    // Workspace events
    WorkspaceProvisioned,
    WorkspaceCleaned,

    // Heartbeat and health
    WorkerHeartbeat,
    WorkerStale,
    WorkerRecovered,

    // Review and merge
    ValidationCompleted,
    ReviewRequested,
    ReviewCompleted,
    MergeRequested,
    MergeCompleted,

    // Dependency events
    DependencyAdded,
    DependencyResolved,
    DependencyBlocked,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::TaskCreated => write!(f, "TASK_CREATED"),
            EventType::TaskTriaged => write!(f, "TASK_TRIAGED"),
            EventType::TaskQueued => write!(f, "TASK_QUEUED"),
            EventType::TaskScheduled => write!(f, "TASK_SCHEDULED"),
            EventType::TaskStarted => write!(f, "TASK_STARTED"),
            EventType::TaskCompleted => write!(f, "TASK_COMPLETED"),
            EventType::TaskFailed => write!(f, "TASK_FAILED"),
            EventType::TaskCancelled => write!(f, "TASK_CANCELLED"),
            EventType::TaskRetried => write!(f, "TASK_RETRIED"),
            EventType::StateChanged => write!(f, "STATE_CHANGED"),
            EventType::RunStarted => write!(f, "RUN_STARTED"),
            EventType::RunCompleted => write!(f, "RUN_COMPLETED"),
            EventType::RunFailed => write!(f, "RUN_FAILED"),
            EventType::WorkerAssigned => write!(f, "WORKER_ASSIGNED"),
            EventType::WorkerReleased => write!(f, "WORKER_RELEASED"),
            EventType::WorkspaceProvisioned => write!(f, "WORKSPACE_PROVISIONED"),
            EventType::WorkspaceCleaned => write!(f, "WORKSPACE_CLEANED"),
            EventType::WorkerHeartbeat => write!(f, "WORKER_HEARTBEAT"),
            EventType::WorkerStale => write!(f, "WORKER_STALE"),
            EventType::WorkerRecovered => write!(f, "WORKER_RECOVERED"),
            EventType::ValidationCompleted => write!(f, "VALIDATION_COMPLETED"),
            EventType::ReviewRequested => write!(f, "REVIEW_REQUESTED"),
            EventType::ReviewCompleted => write!(f, "REVIEW_COMPLETED"),
            EventType::MergeRequested => write!(f, "MERGE_REQUESTED"),
            EventType::MergeCompleted => write!(f, "MERGE_COMPLETED"),
            EventType::DependencyAdded => write!(f, "DEPENDENCY_ADDED"),
            EventType::DependencyResolved => write!(f, "DEPENDENCY_RESOLVED"),
            EventType::DependencyBlocked => write!(f, "DEPENDENCY_BLOCKED"),
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TASK_CREATED" => Ok(EventType::TaskCreated),
            "TASK_TRIAGED" => Ok(EventType::TaskTriaged),
            "TASK_QUEUED" => Ok(EventType::TaskQueued),
            "TASK_SCHEDULED" => Ok(EventType::TaskScheduled),
            "TASK_STARTED" => Ok(EventType::TaskStarted),
            "TASK_COMPLETED" => Ok(EventType::TaskCompleted),
            "TASK_FAILED" => Ok(EventType::TaskFailed),
            "TASK_CANCELLED" => Ok(EventType::TaskCancelled),
            "TASK_RETRIED" => Ok(EventType::TaskRetried),
            "STATE_CHANGED" => Ok(EventType::StateChanged),
            "RUN_STARTED" => Ok(EventType::RunStarted),
            "RUN_COMPLETED" => Ok(EventType::RunCompleted),
            "RUN_FAILED" => Ok(EventType::RunFailed),
            "WORKER_ASSIGNED" => Ok(EventType::WorkerAssigned),
            "WORKER_RELEASED" => Ok(EventType::WorkerReleased),
            "WORKSPACE_PROVISIONED" => Ok(EventType::WorkspaceProvisioned),
            "WORKSPACE_CLEANED" => Ok(EventType::WorkspaceCleaned),
            "WORKER_HEARTBEAT" => Ok(EventType::WorkerHeartbeat),
            "WORKER_STALE" => Ok(EventType::WorkerStale),
            "WORKER_RECOVERED" => Ok(EventType::WorkerRecovered),
            "VALIDATION_COMPLETED" => Ok(EventType::ValidationCompleted),
            "REVIEW_REQUESTED" => Ok(EventType::ReviewRequested),
            "REVIEW_COMPLETED" => Ok(EventType::ReviewCompleted),
            "MERGE_REQUESTED" => Ok(EventType::MergeRequested),
            "MERGE_COMPLETED" => Ok(EventType::MergeCompleted),
            "DEPENDENCY_ADDED" => Ok(EventType::DependencyAdded),
            "DEPENDENCY_RESOLVED" => Ok(EventType::DependencyResolved),
            "DEPENDENCY_BLOCKED" => Ok(EventType::DependencyBlocked),
            _ => Err(format!("Invalid event type: {}", s)),
        }
    }
}

/// A task event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    /// Auto-incremented event ID
    pub id: i64,
    /// Associated task ID
    pub task_id: String,
    /// Associated run ID (optional)
    pub run_id: Option<String>,
    /// Type of event
    pub event_type: EventType,
    /// Event data (JSON)
    pub event_data_json: String,
    /// Event timestamp
    pub created_at: String,
}

/// Input for appending an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEvent {
    /// Task ID
    pub task_id: String,
    /// Run ID (optional)
    pub run_id: Option<String>,
    /// Event type
    pub event_type: EventType,
    /// Event data
    pub data: Option<serde_json::Value>,
}

/// Options for querying events
#[derive(Debug, Clone, Default)]
pub struct EventListOptions {
    /// Filter by task ID
    pub task_id: Option<String>,
    /// Filter by run ID
    pub run_id: Option<String>,
    /// Filter by event type
    pub event_type: Option<Vec<EventType>>,
    /// Maximum results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Order by (asc/desc)
    pub descending: bool,
}
