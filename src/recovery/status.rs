//! Runtime Recovery Status
//!
//! Provides structured health tracking and recovery status for sessions and tasks.
//! Helps identify stuck, crashed, or orphaned executions and guides recovery actions.

use serde::{Deserialize, Serialize};

use crate::store::session::Session;
use crate::store::session::SessionState;

/// Overall health indicator for a session or task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthIndicator {
    /// Session is healthy and making progress
    Healthy,
    /// Session appears stuck (no progress but still alive)
    Stuck,
    /// Session has crashed or terminated unexpectedly
    Crashed,
    /// Unable to determine health
    Unknown,
}

impl HealthIndicator {
    /// Whether this indicates a need for intervention
    pub fn needs_intervention(self) -> bool {
        matches!(self, HealthIndicator::Stuck | HealthIndicator::Crashed)
    }

    /// Human-readable label
    pub fn label(self) -> &'static str {
        match self {
            HealthIndicator::Healthy => "healthy",
            HealthIndicator::Stuck => "stuck",
            HealthIndicator::Crashed => "crashed",
            HealthIndicator::Unknown => "unknown",
        }
    }
}

/// Specific issue detected during health check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssue {
    /// No activity for extended period
    IdleTimeout { idle_secs: u64 },
    /// Session has been running too long
    DurationExceeded { max_secs: u64, actual_secs: u64 },
    /// Too many retries attempted
    RetryExhausted {
        max_retries: i32,
        actual_retries: i32,
    },
    /// Process not responding to heartbeat
    HeartbeatMissed { missed_count: u32 },
    /// Terminal state reached unexpectedly
    UnexpectedTermination,
    /// Workspace or worktree missing
    WorkspaceMissing,
    /// Agent process no longer running
    AgentDied,
}

/// Detailed health assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatus {
    /// Session/task ID being assessed
    pub id: String,
    /// Overall health indicator
    pub health: HealthIndicator,
    /// When the last meaningful activity occurred
    pub last_activity: Option<String>,
    /// Seconds since last activity
    pub idle_secs: u64,
    /// Specific issues detected
    pub issues: Vec<HealthIssue>,
    /// Whether recovery is recommended
    pub recovery_recommended: bool,
    /// Suggested recovery action
    pub recovery_action: Option<RecoveryAction>,
    /// When this assessment was made
    pub assessed_at: String,
}

impl RecoveryStatus {
    /// Create a healthy status
    pub fn healthy(id: String) -> Self {
        Self {
            id,
            health: HealthIndicator::Healthy,
            last_activity: None,
            idle_secs: 0,
            issues: Vec::new(),
            recovery_recommended: false,
            recovery_action: None,
            assessed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Assess a session's health
    pub fn assess_session(
        session: &Session,
        max_idle_secs: u64,
        max_duration_secs: Option<u64>,
        _max_retries: i32,
    ) -> Self {
        let id = session.id.clone();
        let mut issues = Vec::new();
        let mut recovery_recommended = false;
        let mut recovery_action = None;

        let last_update = chrono::DateTime::parse_from_rfc3339(&session.updated_at)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let now = chrono::Utc::now();
        let idle_secs = last_update
            .map(|dt| now.signed_duration_since(dt).num_seconds() as u64)
            .unwrap_or(0);

        let is_terminal = matches!(
            session.state,
            SessionState::Stopped
                | SessionState::Cleaned
                | SessionState::Abandoned
                | SessionState::Crashed
                | SessionState::Failed
        );

        if is_terminal {
            return Self {
                id,
                health: HealthIndicator::Crashed,
                last_activity: Some(session.updated_at.clone()),
                idle_secs,
                issues: vec![HealthIssue::UnexpectedTermination],
                recovery_recommended: false,
                recovery_action: Some(RecoveryAction::Archive),
                assessed_at: now.to_rfc3339(),
            };
        }

        if idle_secs > max_idle_secs {
            issues.push(HealthIssue::IdleTimeout { idle_secs });
            recovery_recommended = true;
            recovery_action = Some(RecoveryAction::CheckAndResume);
        }

        if let Some(max) = max_duration_secs {
            if let Ok(created_dt) = chrono::DateTime::parse_from_rfc3339(&session.created_at) {
                let duration = now.signed_duration_since(created_dt.with_timezone(&chrono::Utc));
                let duration_secs = duration.num_seconds() as u64;
                if duration_secs > max {
                    issues.push(HealthIssue::DurationExceeded {
                        max_secs: max,
                        actual_secs: duration_secs,
                    });
                    recovery_recommended = true;
                    recovery_action = Some(RecoveryAction::InvestigateTimeout);
                }
            }
        }

        let health = if recovery_recommended {
            if issues
                .iter()
                .any(|i| matches!(i, HealthIssue::UnexpectedTermination))
            {
                HealthIndicator::Crashed
            } else {
                HealthIndicator::Stuck
            }
        } else {
            HealthIndicator::Healthy
        };

        Self {
            id,
            health,
            last_activity: Some(session.updated_at.clone()),
            idle_secs,
            issues,
            recovery_recommended,
            recovery_action,
            assessed_at: now.to_rfc3339(),
        }
    }

    /// Check if this status indicates merge can proceed
    pub fn allows_merge(&self) -> bool {
        !matches!(
            self.health,
            HealthIndicator::Crashed | HealthIndicator::Stuck
        ) && !self.recovery_recommended
    }

    /// Get a summary string for logging
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            format!("[{}] healthy", self.id)
        } else {
            let issue_names: Vec<_> = self
                .issues
                .iter()
                .map(|i| match i {
                    HealthIssue::IdleTimeout { .. } => "idle_timeout",
                    HealthIssue::DurationExceeded { .. } => "duration_exceeded",
                    HealthIssue::RetryExhausted { .. } => "retry_exhausted",
                    HealthIssue::HeartbeatMissed { .. } => "heartbeat_missed",
                    HealthIssue::UnexpectedTermination => "unexpected_termination",
                    HealthIssue::WorkspaceMissing => "workspace_missing",
                    HealthIssue::AgentDied => "agent_died",
                })
                .collect();
            format!(
                "[{}] {}: {}",
                self.id,
                self.health.label(),
                issue_names.join(", ")
            )
        }
    }
}

/// Recommended action for recovery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Continue,
    CheckAndResume,
    InvestigateTimeout,
    Escalate,
    Archive,
    RestartFromCheckpoint,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub max_idle_secs: u64,
    pub max_duration_secs: Option<u64>,
    pub max_retries: i32,
    pub check_interval_secs: u64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            max_idle_secs: 300,
            max_duration_secs: Some(3600),
            max_retries: 3,
            check_interval_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::session::{Session, SessionState};

    fn make_session(id: &str, state: SessionState) -> Session {
        Session {
            id: id.to_string(),
            state,
            updated_at: chrono::Utc::now().to_rfc3339(),
            task_id: None,
            provider: "test".to_string(),
            model: "test".to_string(),
            messages: "[]".to_string(),
            token_count: 0,
            summary: None,
            project_path: None,
            parent_session_id: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            metadata: "{}".to_string(),
        }
    }

    #[test]
    fn test_healthy_session() {
        let session = make_session("sess-1", SessionState::Running);
        let status = RecoveryStatus::assess_session(&session, 300, Some(3600), 3);
        assert_eq!(status.health, HealthIndicator::Healthy);
        assert!(!status.recovery_recommended);
        assert!(status.allows_merge());
    }

    #[test]
    fn test_idle_session() {
        let mut session = make_session("sess-2", SessionState::Running);
        session.updated_at = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        let status = RecoveryStatus::assess_session(&session, 300, Some(3600), 3);
        assert_eq!(status.health, HealthIndicator::Stuck);
        assert!(status.recovery_recommended);
    }

    #[test]
    fn test_crashed_session() {
        let session = make_session("sess-3", SessionState::Failed);
        let status = RecoveryStatus::assess_session(&session, 300, Some(3600), 3);
        assert_eq!(status.health, HealthIndicator::Crashed);
    }

    #[test]
    fn test_allows_merge() {
        let healthy = RecoveryStatus::healthy("sess".to_string());
        assert!(healthy.allows_merge());
    }
}
