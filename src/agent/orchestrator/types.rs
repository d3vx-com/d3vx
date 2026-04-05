//! Orchestrator Meta-Agent Types
//!
//! Core types for the coordinator meta-agent that manages multiple concurrent
//! agent sessions: tools, actions, state, and decisions.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Coordinator tool definitions
// ---------------------------------------------------------------------------

/// Tools available to the coordinator meta-agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum CoordinatorTool {
    /// Spawn a new agent session with a prompt.
    LaunchAgent {
        prompt: String,
        branch: Option<String>,
    },
    /// List all active session statuses.
    ListSessions,
    /// Send a nudge message to a running agent session.
    SendNudge { session_id: String, message: String },
    /// Terminate a stuck or unwanted agent session.
    KillSession { session_id: String, reason: String },
    /// Get detailed status for a single session.
    GetStatus { session_id: String },
    /// Launch multiple issues in parallel.
    BatchLaunch {
        issues: Vec<String>,
        max_parallel: usize,
    },
}

// ---------------------------------------------------------------------------
// Coordinator action and decision
// ---------------------------------------------------------------------------

/// A single action chosen by the coordinator with its rationale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinatorAction {
    pub tool: CoordinatorTool,
    pub rationale: String,
}

/// Full decision from the coordinator including confidence and alternatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorDecision {
    pub action: CoordinatorAction,
    pub confidence: f64,
    pub alternatives: Vec<CoordinatorAction>,
}

// ---------------------------------------------------------------------------
// Coordinator state
// ---------------------------------------------------------------------------

/// Current state tracked by the coordinator.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CoordinatorState {
    pub active_sessions: Vec<String>,
    pub pending_reviews: Vec<String>,
    pub stuck_sessions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_tool_serialization() {
        let tool = CoordinatorTool::LaunchAgent {
            prompt: "Fix the bug".to_string(),
            branch: Some("fix/bug-123".to_string()),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: CoordinatorTool = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, tool);
    }

    #[test]
    fn test_coordinator_action() {
        let action = CoordinatorAction {
            tool: CoordinatorTool::ListSessions,
            rationale: "Check current workload".to_string(),
        };
        assert_eq!(action.rationale, "Check current workload");
    }

    #[test]
    fn test_coordinator_decision_confidence() {
        let decision = CoordinatorDecision {
            action: CoordinatorAction {
                tool: CoordinatorTool::KillSession {
                    session_id: "sess-1".to_string(),
                    reason: "stuck".to_string(),
                },
                rationale: "No heartbeat for 10 minutes".to_string(),
            },
            confidence: 0.92,
            alternatives: vec![],
        };
        assert!((decision.confidence - 0.92).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coordinator_state_default() {
        let state = CoordinatorState::default();
        assert!(state.active_sessions.is_empty());
        assert!(state.pending_reviews.is_empty());
        assert!(state.stuck_sessions.is_empty());
    }

    #[test]
    fn test_coordinator_state_serialization() {
        let state = CoordinatorState {
            active_sessions: vec!["sess-1".to_string(), "sess-2".to_string()],
            pending_reviews: vec!["sess-3".to_string()],
            stuck_sessions: vec![],
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: CoordinatorState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn test_send_nudge_tool() {
        let tool = CoordinatorTool::SendNudge {
            session_id: "sess-1".to_string(),
            message: "Please focus on the task".to_string(),
        };
        if let CoordinatorTool::SendNudge {
            session_id,
            message,
        } = tool
        {
            assert_eq!(session_id, "sess-1");
            assert_eq!(message, "Please focus on the task");
        } else {
            panic!("Expected SendNudge variant");
        }
    }

    #[test]
    fn test_batch_launch_tool() {
        let tool = CoordinatorTool::BatchLaunch {
            issues: vec!["ISSUE-1".to_string(), "ISSUE-2".to_string()],
            max_parallel: 3,
        };
        if let CoordinatorTool::BatchLaunch {
            issues,
            max_parallel,
        } = tool
        {
            assert_eq!(issues.len(), 2);
            assert_eq!(max_parallel, 3);
        } else {
            panic!("Expected BatchLaunch variant");
        }
    }
}
