//! Agent restart actions for recovering from failures.
//!
//! Provides strategies and planning for restarting agents based on
//! the nature of the failure that triggered the restart.

use serde::{Deserialize, Serialize};

// ============================================================================
// RESTART STRATEGY
// ============================================================================

/// Strategy to use when restarting an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartStrategy {
    /// Start completely fresh, discarding all context.
    Fresh,
    /// Resume from the last checkpoint before failure.
    FromCheckpoint,
    /// Re-send the last prompt and retry.
    FromLastPrompt,
}

// ============================================================================
// AGENT RESTART
// ============================================================================

/// A request to restart an agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRestart {
    /// Why the restart is needed.
    pub reason: String,
    /// Whether to carry context over into the new session.
    pub preserve_context: bool,
}

impl AgentRestart {
    /// Create a new restart request.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            preserve_context: false,
        }
    }

    /// Set whether to preserve context.
    pub fn with_preserve_context(mut self, preserve: bool) -> Self {
        self.preserve_context = preserve;
        self
    }
}

// ============================================================================
// RESTART RESULT
// ============================================================================

/// Outcome of an agent restart attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartResult {
    /// Whether the restart succeeded.
    pub success: bool,
    /// Session ID of the new session if restart succeeded.
    pub new_session_id: Option<String>,
    /// Human-readable result message.
    pub message: String,
}

impl RestartResult {
    /// Create a successful restart result.
    pub fn ok(new_session_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: true,
            new_session_id: Some(new_session_id.into()),
            message: message.into(),
        }
    }

    /// Create a failed restart result.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            success: false,
            new_session_id: None,
            message: message.into(),
        }
    }
}

// ============================================================================
// RESTART PLANNER
// ============================================================================

/// Decides the restart strategy based on the failure reason.
pub struct RestartPlanner;

impl RestartPlanner {
    /// Choose a restart strategy based on the failure reason.
    ///
    /// - "stuck in loop" or "doom loop": Fresh (context is poisoned).
    /// - "timeout" or "timed out": FromLastPrompt (retry the same prompt).
    /// - "ci failure" or "test failure": FromCheckpoint (before the bad change).
    /// - Anything else: FromLastPrompt as a safe default.
    pub fn plan(reason: &str) -> RestartStrategy {
        let lower = reason.to_lowercase();

        if lower.contains("stuck in loop")
            || lower.contains("doom loop")
            || lower.contains("infinite loop")
        {
            RestartStrategy::Fresh
        } else if lower.contains("timeout") || lower.contains("timed out") {
            RestartStrategy::FromLastPrompt
        } else if lower.contains("ci failure")
            || lower.contains("test failure")
            || lower.contains("build failure")
        {
            RestartStrategy::FromCheckpoint
        } else {
            RestartStrategy::FromLastPrompt
        }
    }

    /// Build a full restart request including the chosen strategy.
    pub fn build_restart(reason: impl Into<String>) -> (AgentRestart, RestartStrategy) {
        let reason = reason.into();
        let strategy = Self::plan(&reason);
        let preserve_context = !matches!(strategy, RestartStrategy::Fresh);
        (
            AgentRestart {
                reason,
                preserve_context,
            },
            strategy,
        )
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_stuck_in_loop() {
        assert_eq!(
            RestartPlanner::plan("Agent stuck in loop"),
            RestartStrategy::Fresh,
        );
    }

    #[test]
    fn test_strategy_doom_loop() {
        assert_eq!(
            RestartPlanner::plan("doom loop detected"),
            RestartStrategy::Fresh,
        );
    }

    #[test]
    fn test_strategy_infinite_loop() {
        assert_eq!(
            RestartPlanner::plan("infinite loop in tool calls"),
            RestartStrategy::Fresh,
        );
    }

    #[test]
    fn test_strategy_timeout() {
        assert_eq!(
            RestartPlanner::plan("Execution timeout after 300s"),
            RestartStrategy::FromLastPrompt,
        );
    }

    #[test]
    fn test_strategy_timed_out() {
        assert_eq!(
            RestartPlanner::plan("Task timed out during review"),
            RestartStrategy::FromLastPrompt,
        );
    }

    #[test]
    fn test_strategy_ci_failure() {
        assert_eq!(
            RestartPlanner::plan("CI failure on branch main"),
            RestartStrategy::FromCheckpoint,
        );
    }

    #[test]
    fn test_strategy_test_failure() {
        assert_eq!(
            RestartPlanner::plan("test failure in integration tests"),
            RestartStrategy::FromCheckpoint,
        );
    }

    #[test]
    fn test_strategy_build_failure() {
        assert_eq!(
            RestartPlanner::plan("build failure in src/main.rs"),
            RestartStrategy::FromCheckpoint,
        );
    }

    #[test]
    fn test_strategy_unknown_defaults_to_last_prompt() {
        assert_eq!(
            RestartPlanner::plan("unknown error"),
            RestartStrategy::FromLastPrompt,
        );
    }

    #[test]
    fn test_build_restart_fresh_no_preserve() {
        let (restart, strategy) = RestartPlanner::build_restart("stuck in loop");
        assert_eq!(strategy, RestartStrategy::Fresh);
        assert!(!restart.preserve_context);
        assert_eq!(restart.reason, "stuck in loop");
    }

    #[test]
    fn test_build_restart_checkpoint_preserves_context() {
        let (restart, strategy) = RestartPlanner::build_restart("CI failure");
        assert_eq!(strategy, RestartStrategy::FromCheckpoint);
        assert!(restart.preserve_context);
    }

    #[test]
    fn test_build_restart_last_prompt_preserves_context() {
        let (restart, strategy) = RestartPlanner::build_restart("timeout");
        assert_eq!(strategy, RestartStrategy::FromLastPrompt);
        assert!(restart.preserve_context);
    }

    #[test]
    fn test_agent_restart_builder() {
        let restart = AgentRestart::new("something went wrong").with_preserve_context(true);
        assert_eq!(restart.reason, "something went wrong");
        assert!(restart.preserve_context);
    }

    #[test]
    fn test_restart_result_ok() {
        let result = RestartResult::ok("sess-42", "Restarted successfully");
        assert!(result.success);
        assert_eq!(result.new_session_id.as_deref(), Some("sess-42"));
        assert_eq!(result.message, "Restarted successfully");
    }

    #[test]
    fn test_restart_result_failed() {
        let result = RestartResult::failed("could not create session");
        assert!(!result.success);
        assert!(result.new_session_id.is_none());
        assert_eq!(result.message, "could not create session");
    }
}
