//! Agent state types and transition validation

use std::time::Duration;

/// Default timeout before transitioning from Active to Idle (5 minutes)
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Duration to consider as "recent" activity for Active state (30 seconds)
pub const ACTIVITY_WINDOW: Duration = Duration::from_secs(30);

/// Granular agent states for activity detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum AgentState {
    /// LLM is generating a response
    Thinking,
    /// A tool is currently executing
    ToolExecution,
    /// No activity for the idle timeout period
    Idle,
    /// Agent is waiting for user input
    WaitingInput,
    /// Agent has finished its task (success or failure)
    Done,
}

impl Default for AgentState {
    fn default() -> Self {
        Self::Idle
    }
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Thinking => write!(f, "Thinking"),
            AgentState::ToolExecution => write!(f, "ToolExecution"),
            AgentState::Idle => write!(f, "Idle"),
            AgentState::WaitingInput => write!(f, "WaitingInput"),
            AgentState::Done => write!(f, "Done"),
        }
    }
}

/// Reason for a state transition.
#[derive(Debug, Clone, serde::Serialize)]
pub enum StateTransitionReason {
    /// Activity detected (streaming or tool execution)
    ActivityDetected,
    /// No activity for the idle timeout period
    IdleTimeout,
    /// Tool requires user input
    ToolRequiresInput { tool_name: String },
    /// User provided input
    UserInputReceived,
    /// Tool failed with a retryable error
    RetryableError { error: String },
    /// External dependency is blocking progress
    ExternalBlocker { description: String },
    /// Blocker was resolved
    BlockerResolved,
    /// Agent loop completed successfully
    CompletedSuccessfully,
    /// Agent loop failed
    Failed { error: String },
    /// Agent was manually stopped
    ManualStop,
    /// Agent was reset
    Reset,
}

impl std::fmt::Display for StateTransitionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateTransitionReason::ActivityDetected => write!(f, "Activity detected"),
            StateTransitionReason::IdleTimeout => write!(f, "Idle timeout"),
            StateTransitionReason::ToolRequiresInput { tool_name } => {
                write!(f, "Tool '{}' requires input", tool_name)
            }
            StateTransitionReason::UserInputReceived => write!(f, "User input received"),
            StateTransitionReason::RetryableError { error } => {
                write!(f, "Retryable error: {}", error)
            }
            StateTransitionReason::ExternalBlocker { description } => {
                write!(f, "External blocker: {}", description)
            }
            StateTransitionReason::BlockerResolved => write!(f, "Blocker resolved"),
            StateTransitionReason::CompletedSuccessfully => write!(f, "Completed successfully"),
            StateTransitionReason::Failed { error } => write!(f, "Failed: {}", error),
            StateTransitionReason::ManualStop => write!(f, "Manual stop"),
            StateTransitionReason::Reset => write!(f, "Reset"),
        }
    }
}

/// Valid state transitions.
/// Returns true if the transition from `from` to `to` is valid.
pub fn is_valid_transition(from: AgentState, to: AgentState) -> bool {
    match from {
        AgentState::Thinking => matches!(
            to,
            AgentState::ToolExecution
                | AgentState::Idle
                | AgentState::WaitingInput
                | AgentState::Done
                | AgentState::Thinking
        ),
        AgentState::ToolExecution => matches!(
            to,
            AgentState::Thinking
                | AgentState::Idle
                | AgentState::WaitingInput
                | AgentState::Done
                | AgentState::ToolExecution
        ),
        AgentState::Idle => matches!(
            to,
            AgentState::Thinking | AgentState::ToolExecution | AgentState::Done
        ),
        AgentState::WaitingInput => matches!(to, AgentState::Thinking | AgentState::Done),
        AgentState::Done => false, // Terminal state
    }
}
