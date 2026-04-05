//! Result, error, and event types for the agent loop.

use serde::Serialize;

use crate::providers::{ProviderError, StopReason, TokenUsage};

/// Result of running the agent loop.
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Accumulated text response.
    pub text: String,
    /// Total token usage.
    pub usage: TokenUsage,
    /// Number of tool calls made.
    pub tool_calls: u32,
    /// Number of iterations.
    pub iterations: u32,
    /// Whether the agent formally completed via complete_task tool.
    pub task_completed: bool,
}

/// Internal outcome for program step execution.
pub(super) enum ProgramStepOutcome {
    ProceedToProvider,
    Consumed,
    Stop,
}

/// Agent event data
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent started.
    Start { session_id: String },
    /// Agent thinking.
    Thinking { text: String },
    /// Agent text response generated.
    Text { text: String },
    /// Tool use started.
    ToolStart { id: String, name: String },
    /// Tool input received.
    ToolInput { json: String },
    /// Tool execution completed.
    ToolEnd {
        id: String,
        name: String,
        result: String,
        is_error: bool,
        elapsed_ms: u64,
    },
    /// Message turn ended.
    MessageEnd {
        usage: TokenUsage,
        stop_reason: StopReason,
    },
    /// Iteration completed.
    IterationEnd { iteration: u32, usage: TokenUsage },
    /// Agent completed the task.
    Done {
        final_text: String,
        iterations: u32,
        tool_calls: u32,
        total_usage: TokenUsage,
    },
    /// Sub-agent requested.
    SubAgentSpawn { task: String },
    /// Agent is waiting for tool approval.
    WaitingApproval { id: String, name: String },
    /// Error occurred.
    Error { error: String },
    /// Agent state changed.
    StateChange {
        old_state: super::super::state::AgentState,
        new_state: super::super::state::AgentState,
    },
    /// Agent loop finished.
    Finished {
        iterations: u32,
        tool_calls: u32,
        total_usage: TokenUsage,
    },
    /// Resource cleanup performed.
    Cleanup { pruned_count: usize },
}

/// Error type for agent loop operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentLoopError {
    #[error("Provider error: {0}")]
    ProviderError(#[from] ProviderError),

    #[error("Tool coordinator error: {0}")]
    ToolCoordinatorError(#[from] crate::agent::tool_coordinator::ToolCoordinatorError),

    #[error("Context window exceeded")]
    ContextExceeded,

    #[error("Max iterations reached")]
    MaxIterationsReached,

    #[error("Aborted by user")]
    Aborted,

    #[error("Loop detected: {0}")]
    LoopDetected(String),
}
