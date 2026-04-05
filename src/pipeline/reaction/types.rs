//! Event, result, and core types for the reaction engine.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================================
// EVENT TYPES
// ============================================================================

/// External events that trigger reactions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReactionEvent {
    /// CI pipeline failure
    CIFailure {
        /// Repository name (owner/repo)
        repository: String,
        /// Branch name
        branch: String,
        /// Commit SHA
        commit_sha: String,
        /// CI context (e.g., "ci/tests")
        context: String,
        /// Error description
        description: String,
        /// URL to CI details
        target_url: Option<String>,
        /// Associated task ID if any
        task_id: Option<String>,
    },

    /// PR review comment received
    ReviewComment {
        /// PR number
        pr_number: u64,
        /// Repository name
        repository: String,
        /// Comment author
        author: String,
        /// Comment body
        body: String,
        /// Whether changes were requested
        changes_requested: bool,
        /// Associated task ID if any
        task_id: Option<String>,
    },

    /// Merge conflict detected
    MergeConflict {
        /// Worktree path
        worktree_path: String,
        /// Base branch
        base_branch: String,
        /// Conflicted files
        conflicted_files: Vec<String>,
        /// Associated task ID
        task_id: String,
    },

    /// Agent appears stuck (idle too long)
    AgentIdle {
        /// Worker ID
        worker_id: u64,
        /// Task ID being worked on
        task_id: String,
        /// Time since last activity
        idle_duration_secs: u64,
        /// Last known phase
        last_phase: Option<String>,
    },

    /// Task execution failed
    TaskFailed {
        /// Task ID
        task_id: String,
        /// Error message
        error: String,
        /// Phase where failure occurred
        failed_phase: Option<crate::pipeline::phases::Phase>,
        /// Number of retry attempts so far
        retry_count: u32,
    },
}

impl ReactionEvent {
    /// Get the associated task ID if any
    pub fn task_id(&self) -> Option<&str> {
        match self {
            ReactionEvent::CIFailure { task_id, .. } => task_id.as_deref(),
            ReactionEvent::ReviewComment { task_id, .. } => task_id.as_deref(),
            ReactionEvent::MergeConflict { task_id, .. } => Some(task_id),
            ReactionEvent::AgentIdle { task_id, .. } => Some(task_id),
            ReactionEvent::TaskFailed { task_id, .. } => Some(task_id),
        }
    }

    /// Get a human-readable event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            ReactionEvent::CIFailure { .. } => "ci_failure",
            ReactionEvent::ReviewComment { .. } => "review_comment",
            ReactionEvent::MergeConflict { .. } => "merge_conflict",
            ReactionEvent::AgentIdle { .. } => "agent_idle",
            ReactionEvent::TaskFailed { .. } => "task_failed",
        }
    }
}

// ============================================================================
// REACTION TYPES
// ============================================================================

/// Types of reactions the engine can take
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionType {
    /// Attempt automatic fix
    AutoFix,
    /// Notify human for intervention
    Notify,
    /// Escalate to higher priority
    Escalate,
    /// Create checkpoint for recovery
    Checkpoint,
    /// No action needed
    NoAction,
    /// Cancel the task
    Cancel,
}

impl fmt::Display for ReactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReactionType::AutoFix => write!(f, "auto_fix"),
            ReactionType::Notify => write!(f, "notify"),
            ReactionType::Escalate => write!(f, "escalate"),
            ReactionType::Checkpoint => write!(f, "checkpoint"),
            ReactionType::NoAction => write!(f, "no_action"),
            ReactionType::Cancel => write!(f, "cancel"),
        }
    }
}

// ============================================================================
// HANDLER DECISION (intermediate - avoids cloning event per return path)
// ============================================================================

/// Decision returned by a handler method (no event clone needed).
/// Constructed once per handler invocation, then wrapped into ReactionResult
/// by process_event with the owned event.
pub(super) struct HandlerDecision {
    pub reaction: ReactionType,
    pub reason: String,
    pub metadata: HashMap<String, String>,
}

impl HandlerDecision {
    pub fn new(reaction: ReactionType, reason: String) -> Self {
        Self {
            reaction,
            reason,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

// ============================================================================
// REACTION RESULT
// ============================================================================

/// Result of processing a reaction event
#[derive(Debug, Clone)]
pub struct ReactionResult {
    /// The original event
    pub event: ReactionEvent,
    /// The determined reaction type
    pub reaction: ReactionType,
    /// Reason for this reaction
    pub reason: String,
    /// Whether the reaction was executed successfully
    pub executed: bool,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Additional metadata about the reaction
    pub metadata: HashMap<String, String>,
}

impl ReactionResult {
    /// Create a new reaction result
    pub fn new(event: ReactionEvent, reaction: ReactionType, reason: String) -> Self {
        Self {
            event,
            reaction,
            reason,
            executed: false,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Mark as executed successfully
    pub fn with_executed(mut self) -> Self {
        self.executed = true;
        self
    }

    /// Add an error
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

// ============================================================================
// REACTION AUDIT TRAIL
// ============================================================================

/// Record of a reaction for audit purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionAuditRecord {
    /// Unique record ID
    pub id: String,
    /// Timestamp of the reaction
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// The event that triggered the reaction
    pub event: ReactionEvent,
    /// The reaction taken
    pub reaction: ReactionType,
    /// Reason for the reaction
    pub reason: String,
    /// Whether the reaction was successful
    pub success: bool,
    /// Associated task ID
    pub task_id: Option<String>,
    /// Retry count at time of reaction
    pub retry_count: u32,
}

// ============================================================================
// STATISTICS
// ============================================================================

/// Statistics about reaction engine activity
#[derive(Debug, Clone, Default)]
pub struct ReactionStats {
    /// Total events processed
    pub total_events: u64,
    /// Auto-fix attempts
    pub auto_fix_attempts: u64,
    /// Successful auto-fixes
    pub auto_fix_successes: u64,
    /// Notifications sent
    pub notifications_sent: u64,
    /// Escalations
    pub escalations: u64,
    /// Checkpoints created
    pub checkpoints_created: u64,
    /// Tasks cancelled
    pub tasks_cancelled: u64,
}
