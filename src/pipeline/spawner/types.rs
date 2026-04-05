//! Spawner Types
//!
//! Core types for the batch issue-to-agent launch system.

/// Configuration for how issues are launched as agent sessions.
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Prefix for generated branch names.
    pub branch_prefix: String,
    /// Maximum number of concurrent launches.
    pub max_concurrent: usize,
    /// Whether to automatically create a PR after the agent finishes.
    pub auto_create_pr: bool,
    /// Override the agent model for this launch.
    pub agent_model: Option<String>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            branch_prefix: "d3vx".to_string(),
            max_concurrent: 3,
            auto_create_pr: false,
            agent_model: None,
        }
    }
}

/// Issue context from an external tracker.
#[derive(Debug, Clone)]
pub struct IssueContext {
    /// Issue identifier (e.g., "42", "PROJ-123").
    pub id: String,
    /// Issue title.
    pub title: String,
    /// Issue body / description.
    pub body: String,
    /// Labels attached to the issue.
    pub labels: Vec<String>,
    /// Which tracker this issue came from.
    pub tracker: TrackerKind,
}

/// Supported issue tracker backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerKind {
    GitHub,
    Linear,
    Jira,
    Custom,
}

impl std::fmt::Display for TrackerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "GitHub"),
            Self::Linear => write!(f, "Linear"),
            Self::Jira => write!(f, "Jira"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

/// Result of a single issue launch attempt.
#[derive(Debug, Clone)]
pub struct SpawnResult {
    /// Session ID for the launched agent.
    pub session_id: String,
    /// Branch name created for this issue.
    pub branch: String,
    /// Launch status.
    pub status: SpawnStatus,
}

/// Status of an individual spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnStatus {
    /// Successfully launched.
    Launched,
    /// Skipped with a reason.
    Skipped { reason: String },
    /// Failed with an error.
    Failed { error: String },
}

/// Strategy for generating branch names from issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchSpec {
    /// Derive from issue title, truncated to max_length.
    FromTitle { max_length: usize },
    /// Use the issue ID directly.
    FromIssueId,
    /// Apply a template pattern (e.g., "{prefix}/{id}-{title}").
    Template { pattern: String },
}
