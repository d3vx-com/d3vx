//! Agent Loop
//!
//! The central orchestration loop that handles the conversation with the LLM,
//! processes tool calls, and manages the agent's state.

mod config;
mod execution;
mod lifecycle;
mod lsp_inject;
mod messages;
mod pacing;
mod program_steps;
mod retry;
mod stream;
mod tools_exec;
mod types;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::providers::TokenUsage;

use super::state::AgentStateTracker;
use super::tool_coordinator::ToolCoordinator;

// Re-export public types
pub use config::{AgentConfig, DEFAULT_MAX_ITERATIONS};
pub use types::{AgentEvent, AgentLoopError, AgentResult};

/// The main agent loop that orchestrates conversations with the LLM.
pub struct AgentLoop {
    /// LLM provider.
    pub(crate) provider: Arc<dyn crate::providers::Provider>,
    /// Conversation manager.
    pub conversation: Arc<RwLock<super::conversation::Conversation>>,
    /// Tool coordinator.
    tools: Arc<ToolCoordinator>,
    /// Agent configuration.
    pub config: RwLock<AgentConfig>,
    /// Event sender.
    broadcast_tx: broadcast::Sender<AgentEvent>,
    /// Accumulated token usage.
    total_usage: Arc<RwLock<TokenUsage>>,
    /// Guard for tool execution approval.
    pub guard: Option<Arc<super::guard::CommandGuard>>,
    /// State tracker for granular agent states.
    pub state_tracker: AgentStateTracker,
    /// Multi-persistence logger
    logger: Option<super::logger::JsonlLogger>,
    /// Recovery escalation strategy
    recovery_strategy: crate::recovery::EscalationStrategy,
    /// Consecutive failure count for recovery logic
    failure_count: Arc<RwLock<u32>>,
    /// Whether the agent is currently paused.
    pub paused: Arc<RwLock<bool>>,
    /// Optional programmatic execution plan for this run.
    step_controller: Arc<Mutex<Option<super::step_controller::StepController>>>,
    /// File change log for undo/revert support.
    file_change_log: Arc<Mutex<super::file_change_log::FileChangeLog>>,
    /// Optional LSP bridge for fast per-file diagnostics.
    lsp_bridge: Option<Arc<crate::lsp::LspBridge>>,
}

impl AgentLoop {
    /// Create a new agent loop.
    pub fn new(
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<ToolCoordinator>,
        guard: Option<Arc<super::guard::CommandGuard>>,
        config: AgentConfig,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        let session_id = config.session_id.clone();

        Self {
            provider,
            conversation: Arc::new(RwLock::new(super::conversation::Conversation::new())),
            tools,
            guard,
            config: RwLock::new(config.clone()),
            broadcast_tx: broadcast_tx.clone(),
            total_usage: Arc::new(RwLock::new(TokenUsage::default())),
            paused: Arc::new(RwLock::new(false)),
            state_tracker: {
                let current_tx = broadcast_tx.clone();
                AgentStateTracker::new().with_state_change_callback(Arc::new(
                    move |old_state, new_state, _reason| {
                        let _ = current_tx.send(AgentEvent::StateChange {
                            old_state,
                            new_state,
                        });
                    },
                ))
            },
            logger: Some(super::logger::JsonlLogger::new(&session_id)),
            recovery_strategy: crate::recovery::EscalationStrategy {
                max_retries: config.recovery.max_retries,
                initial_delay: Duration::from_millis(config.recovery.initial_delay_ms as u64),
                max_delay: Duration::from_millis(config.recovery.max_delay_ms as u64),
                backoff_multiplier: config.recovery.backoff_multiplier,
                checkpoint_enabled: config.recovery.checkpoint_enabled,
            },
            failure_count: Arc::new(RwLock::new(0)),
            step_controller: Arc::new(Mutex::new(None)),
            file_change_log: Arc::new(Mutex::new(super::file_change_log::FileChangeLog::new())),
            lsp_bridge: None,
        }
    }

    /// Create a new agent loop with an event channel.
    pub fn with_events(
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<ToolCoordinator>,
        guard: Option<Arc<super::guard::CommandGuard>>,
        config: AgentConfig,
    ) -> (Self, broadcast::Receiver<AgentEvent>) {
        let agent = Self::new(provider, tools, guard, config);
        let receiver = agent.subscribe();
        (agent, receiver)
    }

    /// Create a new agent loop with a shared conversation (for inline agents).
    ///
    /// This allows inline agents to share the parent's conversation directly,
    /// avoiding the overhead of separate message histories and serialization.
    pub fn with_shared_conversation(
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<ToolCoordinator>,
        guard: Option<Arc<super::guard::CommandGuard>>,
        config: AgentConfig,
        conversation: Arc<RwLock<super::conversation::Conversation>>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        let session_id = config.session_id.clone();

        Self {
            provider,
            conversation,
            tools,
            guard,
            config: RwLock::new(config.clone()),
            broadcast_tx: broadcast_tx.clone(),
            total_usage: Arc::new(RwLock::new(TokenUsage::default())),
            paused: Arc::new(RwLock::new(false)),
            state_tracker: {
                let current_tx = broadcast_tx.clone();
                AgentStateTracker::new().with_state_change_callback(Arc::new(
                    move |old_state, new_state, _reason| {
                        let _ = current_tx.send(AgentEvent::StateChange {
                            old_state,
                            new_state,
                        });
                    },
                ))
            },
            logger: Some(super::logger::JsonlLogger::new(&session_id)),
            recovery_strategy: crate::recovery::EscalationStrategy {
                max_retries: config.recovery.max_retries,
                initial_delay: Duration::from_millis(config.recovery.initial_delay_ms as u64),
                max_delay: Duration::from_millis(config.recovery.max_delay_ms as u64),
                backoff_multiplier: config.recovery.backoff_multiplier,
                checkpoint_enabled: config.recovery.checkpoint_enabled,
            },
            failure_count: Arc::new(RwLock::new(0)),
            step_controller: Arc::new(Mutex::new(None)),
            file_change_log: Arc::new(Mutex::new(
                crate::agent::file_change_log::FileChangeLog::new(),
            )),
            lsp_bridge: None,
        }
    }

    /// Attach the LSP bridge for fast diagnostic injection.
    pub fn with_lsp_bridge(mut self, bridge: Arc<crate::lsp::LspBridge>) -> Self {
        self.lsp_bridge = Some(bridge);
        self
    }
}

#[cfg(test)]
mod tests;
