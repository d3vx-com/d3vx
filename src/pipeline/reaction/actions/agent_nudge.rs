//! Agent nudge actions for sending targeted messages to running agents.
//!
//! A "nudge" is a lightweight, actionable message delivered to an agent
//! to steer its behavior without requiring a full restart or human
//! intervention.

use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================================
// NUDGE PRIORITY
// ============================================================================

/// Priority level for an agent nudge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgePriority {
    /// Informational hint, agent may ignore.
    Low,
    /// Standard priority, agent should acknowledge.
    Normal,
    /// Urgent, agent must act immediately.
    Urgent,
}

impl fmt::Display for NudgePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NudgePriority::Low => write!(f, "low"),
            NudgePriority::Normal => write!(f, "normal"),
            NudgePriority::Urgent => write!(f, "urgent"),
        }
    }
}

impl Default for NudgePriority {
    fn default() -> Self {
        NudgePriority::Normal
    }
}

// ============================================================================
// AGENT NUDGE
// ============================================================================

/// A targeted message delivered to a running agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNudge {
    /// The message body to deliver.
    pub message: String,
    /// Optional additional context (e.g. log excerpt, file path).
    pub context: Option<String>,
    /// Priority of the nudge.
    pub priority: NudgePriority,
}

impl AgentNudge {
    /// Create a new nudge with a message and default priority.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: None,
            priority: NudgePriority::Normal,
        }
    }

    /// Attach additional context.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: NudgePriority) -> Self {
        self.priority = priority;
        self
    }
}

// ============================================================================
// AGENT NUDGE RESULT
// ============================================================================

/// Outcome of delivering a nudge to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNudgeResult {
    /// Whether the nudge was successfully delivered.
    pub delivered: bool,
    /// Whether the agent acknowledged and accepted the nudge.
    pub accepted: bool,
    /// Optional free-form response from the agent.
    pub response: Option<String>,
}

impl AgentNudgeResult {
    /// Create a successful delivery result.
    pub fn delivered(accepted: bool, response: Option<String>) -> Self {
        Self {
            delivered: true,
            accepted,
            response,
        }
    }

    /// Create a failed delivery result.
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            delivered: false,
            accepted: false,
            response: Some(reason.into()),
        }
    }
}

// ============================================================================
// NUDGE COMPOSER
// ============================================================================

/// Builds pre-templated nudges for common reaction scenarios.
pub struct NudgeComposer;

impl NudgeComposer {
    /// Compose a nudge for a CI failure requiring agent attention.
    pub fn ci_fix_nudge(check_name: &str, log_snippet: &str) -> AgentNudge {
        AgentNudge::new(format!(
            "CI check '{}' failed. Please investigate and fix the issue.",
            check_name,
        ))
        .with_context(format!("Recent CI output:\n{}", log_snippet))
        .with_priority(NudgePriority::Urgent)
    }

    /// Compose a nudge from a PR review comment.
    pub fn review_feedback_nudge(comment: &str, file: &str) -> AgentNudge {
        AgentNudge::new(format!(
            "Review feedback on '{}': please address the comment and apply changes.",
            file,
        ))
        .with_context(format!("Comment:\n{}", comment))
        .with_priority(NudgePriority::Normal)
    }

    /// Compose a nudge to recover an agent that appears stuck.
    pub fn stuck_recovery_nudge(last_action: &str, duration_mins: u64) -> AgentNudge {
        AgentNudge::new(format!(
            "You have been idle for {} minute(s) after: '{}'. \
             Please continue with the next step or report progress.",
            duration_mins, last_action,
        ))
        .with_priority(if duration_mins > 30 {
            NudgePriority::Urgent
        } else {
            NudgePriority::Normal
        })
    }

    /// Compose a nudge to help resolve merge conflicts.
    pub fn conflict_resolve_nudge(conflicting_files: Vec<String>) -> AgentNudge {
        let file_list = conflicting_files.join(", ");
        AgentNudge::new(format!(
            "Merge conflict detected in {} file(s). \
             Please resolve the conflicts and ensure tests pass.",
            conflicting_files.len(),
        ))
        .with_context(format!("Conflicting files: {}", file_list))
        .with_priority(NudgePriority::Urgent)
    }

    /// Compose a nudge warning the agent about an approaching timeout.
    pub fn timeout_warning_nudge(elapsed_mins: u64, phase: &str) -> AgentNudge {
        AgentNudge::new(format!(
            "Task has been in '{}' phase for {} minute(s). \
             Please wrap up or checkpoint progress soon.",
            phase, elapsed_mins,
        ))
        .with_priority(NudgePriority::Normal)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_fix_nudge() {
        let nudge = NudgeComposer::ci_fix_nudge("ci/tests", "assertion failed: 1 == 2");
        assert!(nudge.message.contains("ci/tests"));
        assert!(nudge.context.unwrap().contains("assertion failed"));
        assert_eq!(nudge.priority, NudgePriority::Urgent);
    }

    #[test]
    fn test_review_feedback_nudge() {
        let nudge = NudgeComposer::review_feedback_nudge("Please fix the typo", "src/lib.rs");
        assert!(nudge.message.contains("src/lib.rs"));
        assert!(nudge.context.unwrap().contains("Please fix the typo"));
        assert_eq!(nudge.priority, NudgePriority::Normal);
    }

    #[test]
    fn test_stuck_recovery_nudge_normal() {
        let nudge = NudgeComposer::stuck_recovery_nudge("ran tests", 10);
        assert!(nudge.message.contains("10 minute(s)"));
        assert!(nudge.message.contains("ran tests"));
        assert_eq!(nudge.priority, NudgePriority::Normal);
    }

    #[test]
    fn test_stuck_recovery_nudge_urgent() {
        let nudge = NudgeComposer::stuck_recovery_nudge("compiled project", 45);
        assert_eq!(nudge.priority, NudgePriority::Urgent);
    }

    #[test]
    fn test_conflict_resolve_nudge() {
        let files = vec!["src/lib.rs".to_string(), "Cargo.toml".to_string()];
        let nudge = NudgeComposer::conflict_resolve_nudge(files);
        assert!(nudge.message.contains("2 file(s)"));
        let ctx = nudge.context.unwrap();
        assert!(ctx.contains("src/lib.rs"));
        assert!(ctx.contains("Cargo.toml"));
        assert_eq!(nudge.priority, NudgePriority::Urgent);
    }

    #[test]
    fn test_timeout_warning_nudge() {
        let nudge = NudgeComposer::timeout_warning_nudge(25, "implement");
        assert!(nudge.message.contains("implement"));
        assert!(nudge.message.contains("25 minute(s)"));
        assert_eq!(nudge.priority, NudgePriority::Normal);
    }

    #[test]
    fn test_agent_nudge_builder() {
        let nudge = AgentNudge::new("hello")
            .with_context("extra info")
            .with_priority(NudgePriority::Low);
        assert_eq!(nudge.message, "hello");
        assert_eq!(nudge.context.as_deref(), Some("extra info"));
        assert_eq!(nudge.priority, NudgePriority::Low);
    }

    #[test]
    fn test_nudge_result_delivered() {
        let result = AgentNudgeResult::delivered(true, Some("working on it".to_string()));
        assert!(result.delivered);
        assert!(result.accepted);
        assert_eq!(result.response.as_deref(), Some("working on it"));
    }

    #[test]
    fn test_nudge_result_failed() {
        let result = AgentNudgeResult::failed("agent not reachable");
        assert!(!result.delivered);
        assert!(!result.accepted);
        assert_eq!(result.response.as_deref(), Some("agent not reachable"));
    }

    #[test]
    fn test_nudge_priority_display() {
        assert_eq!(NudgePriority::Low.to_string(), "low");
        assert_eq!(NudgePriority::Normal.to_string(), "normal");
        assert_eq!(NudgePriority::Urgent.to_string(), "urgent");
    }

    #[test]
    fn test_nudge_priority_default() {
        assert_eq!(NudgePriority::default(), NudgePriority::Normal);
    }
}
