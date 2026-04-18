//! Agent Session
//!
//! Manages a standalone agent session that bridges the agent loop
//! with the TUI via IPC-like events.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use super::agent_loop::{AgentConfig, AgentEvent, AgentLoop};
use super::tool_coordinator::ToolCoordinator;
use crate::ipc::types::{Message, MessageRole, ThinkingState, TokenUsage, ToolCall, ToolStatus};
use crate::providers::anthropic::AnthropicProvider;
use crate::providers::Provider;

/// Handle to control the agent session
#[derive(Clone)]
pub struct AgentSessionHandle {
    /// Sender for user messages
    message_tx: mpsc::Sender<String>,
    /// Sender for cancel signal
    cancel_tx: mpsc::Sender<()>,
}

impl AgentSessionHandle {
    /// Send a message to the agent
    pub async fn send_message(&self, content: &str) -> anyhow::Result<()> {
        self.message_tx.send(content.to_string()).await?;
        Ok(())
    }

    /// Cancel the current agent operation
    pub async fn cancel(&self) -> anyhow::Result<()> {
        self.cancel_tx.send(()).await?;
        Ok(())
    }
}

/// Configuration for the agent session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// API key for the provider
    pub api_key: String,
    /// Optional base URL override (for proxies like z.ai)
    pub base_url: Option<String>,
    /// Model to use
    pub model: String,
    /// System prompt
    pub system_prompt: String,
    /// Working directory
    pub working_dir: String,
    /// Session ID
    pub session_id: String,
    /// Permission configuration
    pub permissions: crate::config::types::PermissionsConfig,
}

impl Default for SessionConfig {
    fn default() -> Self {
        let working_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        Self {
            api_key: String::new(),
            base_url: None,
            model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: crate::agent::build_system_prompt(&working_dir, None),
            working_dir,
            session_id: uuid::Uuid::new_v4().to_string(),
            permissions: crate::config::types::PermissionsConfig::default(),
        }
    }
}

/// Create a standalone agent session.
///
/// Returns a handle to control the session and a receiver for events.
pub fn create_agent_session(
    config: SessionConfig,
) -> (AgentSessionHandle, mpsc::Receiver<SessionEvent>) {
    let (message_tx, message_rx) = mpsc::channel(100);
    let (cancel_tx, cancel_rx) = mpsc::channel(1);
    let (event_tx, event_rx) = mpsc::channel(100);

    let handle = AgentSessionHandle {
        message_tx,
        cancel_tx,
    };

    // Spawn the session task
    tokio::spawn(async move {
        if let Err(e) = run_session(config, message_rx, cancel_rx, event_tx).await {
            error!(error = %e, "Session error");
        }
    });

    (handle, event_rx)
}

/// Events emitted by the agent session
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A new message was created or updated
    Message(Message),
    /// A tool call was created or updated
    ToolCall(ToolCall),
    /// Thinking state changed
    Thinking(ThinkingState),
    /// Session ended with token usage
    SessionEnd(TokenUsage),
    /// Error occurred
    Error(String),
    /// Agent is waiting for tool approval
    WaitingApproval { id: String, name: String },
}

/// Internal state for tracking the current assistant message
struct StreamingState {
    /// Current assistant message ID
    message_id: Option<String>,
    /// Accumulated text content
    text: String,
    /// Tool calls in progress
    tool_calls: std::collections::HashMap<String, ToolCall>,
}

impl Default for StreamingState {
    fn default() -> Self {
        Self {
            message_id: None,
            text: String::new(),
            tool_calls: std::collections::HashMap::new(),
        }
    }
}

/// Run the agent session loop
async fn run_session(
    config: SessionConfig,
    mut message_rx: mpsc::Receiver<String>,
    mut cancel_rx: mpsc::Receiver<()>,
    event_tx: mpsc::Sender<SessionEvent>,
) -> anyhow::Result<()> {
    info!(session_id = %config.session_id, "Starting agent session");

    // Create provider with base_url override if configured
    let provider: Arc<dyn Provider> = {
        let options = crate::providers::ProviderOptions {
            base_url: config.base_url.clone(),
            ..Default::default()
        };
        Arc::new(AnthropicProvider::with_options(config.api_key, options))
    };

    // Create tool coordinator
    let tools = Arc::new(ToolCoordinator::new());

    // Create agent config
    let agent_config = AgentConfig {
        model: config.model,
        system_prompt: config.system_prompt,
        working_dir: config.working_dir,
        session_id: config.session_id.clone(),
        ..Default::default()
    };

    // Create command guard
    let guard = Arc::new(super::guard::CommandGuard::new(
        config.permissions.clone(),
        config.session_id.clone(),
    ));

    // Create agent loop with events
    let (agent, mut agent_events) =
        AgentLoop::with_events(provider, tools, Some(guard.clone()), agent_config);

    // State for the current assistant message
    let streaming_state: Arc<RwLock<StreamingState>> =
        Arc::new(RwLock::new(StreamingState::default()));

    // Clone for the event processor task
    let event_tx_for_events = event_tx.clone();
    let streaming_state_for_events = streaming_state.clone();

    // Spawn task to process agent events and forward to session events
    tokio::spawn(async move {
        loop {
            match agent_events.recv().await {
                Ok(event) => {
                    process_agent_event(event, &event_tx_for_events, &streaming_state_for_events)
                        .await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("Agent event channel closed");
                    break;
                }
                Err(e) => {
                    debug!("Agent event channel error: {}", e);
                    // Lagging is okay, we just skip some events
                }
            }
        }
    });

    // Main loop - wait for user messages and run agent
    loop {
        tokio::select! {
            // Check for cancel signal
            _ = cancel_rx.recv() => {
                info!("Session cancelled");
                agent.pause().await;
                break;
            }

            // Wait for user message
            Some(content) = message_rx.recv() => {
                // Add user message to conversation
                let user_msg = Message::user(&content);

                // Send message event
                let _ = event_tx.send(SessionEvent::Message(user_msg)).await;

                // Reset streaming state for new response
                {
                    let mut state = streaming_state.write().await;
                    state.message_id = None;
                    state.text.clear();
                    state.tool_calls.clear();
                }

                // Add to agent
                agent.add_user_message(&content).await;

                // Run agent synchronously (the events are processed in the background task)
                match agent.run().await {
                    Ok(result) => {
                        if let Some(reason) = result.safety_stop_reason() {
                            error!(reason = %reason, "Agent stopped for safety");
                            let _ = event_tx
                                .send(SessionEvent::Error(format!(
                                    "Agent stopped for safety: {reason}"
                                )))
                                .await;
                        } else {
                            info!(
                                iterations = result.iterations,
                                tool_calls = result.tool_calls,
                                input_tokens = result.usage.input_tokens,
                                output_tokens = result.usage.output_tokens,
                                "Agent turn completed"
                            );
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Agent error");
                        let _ = event_tx.send(SessionEvent::Error(e.to_string())).await;
                    }
                }
            }

            else => break,
        }
    }

    Ok(())
}

/// Process a single agent event and convert to session events
async fn process_agent_event(
    event: AgentEvent,
    event_tx: &mpsc::Sender<SessionEvent>,
    streaming_state: &Arc<RwLock<StreamingState>>,
) {
    match event {
        AgentEvent::Text { text } => {
            let mut state = streaming_state.write().await;
            state.text.push_str(&text);

            // Create or get message ID
            let msg_id = state
                .message_id
                .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
                .clone();

            let msg = Message {
                id: msg_id,
                role: MessageRole::Assistant,
                content: state.text.clone(),
                timestamp: chrono::Utc::now(),
                is_error: false,
                tool_calls: state.tool_calls.values().cloned().collect(),
                is_streaming: true,
                shell_cmd: None,
                exit_code: None,
            };

            let _ = event_tx.send(SessionEvent::Message(msg)).await;
        }

        AgentEvent::Thinking { text } => {
            let state = ThinkingState {
                is_thinking: true,
                text,
                ..Default::default()
            };
            let _ = event_tx.send(SessionEvent::Thinking(state)).await;
        }

        AgentEvent::ToolStart { id, name } => {
            let mut state = streaming_state.write().await;
            let tool_call = ToolCall {
                id: id.clone(),
                name,
                input: serde_json::Value::Null,
                status: ToolStatus::Running,
                output: None,
                elapsed: None,
            };
            state.tool_calls.insert(id.clone(), tool_call.clone());

            let _ = event_tx.send(SessionEvent::ToolCall(tool_call)).await;
        }

        AgentEvent::ToolInput { json } => {
            // Tool input is being streamed - we could accumulate this
            debug!(json = %json, "Tool input delta");
        }

        AgentEvent::ToolEnd {
            id,
            name,
            result,
            is_error,
            elapsed_ms,
        } => {
            let mut state = streaming_state.write().await;

            let tool_call = ToolCall {
                id: id.clone(),
                name,
                input: serde_json::Value::Null,
                status: if is_error {
                    ToolStatus::Error
                } else {
                    ToolStatus::Completed
                },
                output: Some(result),
                elapsed: Some(elapsed_ms),
            };

            state.tool_calls.insert(id.clone(), tool_call.clone());

            // Update the message with the new tool call
            if let Some(msg_id) = state.message_id.clone() {
                let msg = Message {
                    id: msg_id,
                    role: MessageRole::Assistant,
                    content: state.text.clone(),
                    timestamp: chrono::Utc::now(),
                    is_error: false,
                    tool_calls: state.tool_calls.values().cloned().collect(),
                    is_streaming: true,
                    shell_cmd: None,
                    exit_code: None,
                };
                let _ = event_tx.send(SessionEvent::Message(msg)).await;
            }

            let _ = event_tx.send(SessionEvent::ToolCall(tool_call)).await;
        }

        AgentEvent::MessageEnd { usage, stop_reason } => {
            let mut state = streaming_state.write().await;

            // Mark message as complete
            if let Some(msg_id) = state.message_id.clone() {
                let msg = Message {
                    id: msg_id,
                    role: MessageRole::Assistant,
                    content: state.text.clone(),
                    timestamp: chrono::Utc::now(),
                    is_error: false,
                    tool_calls: state.tool_calls.values().cloned().collect(),
                    is_streaming: false,
                    shell_cmd: None,
                    exit_code: None,
                };
                let _ = event_tx.send(SessionEvent::Message(msg)).await;
            }

            // Reset streaming state for next message
            state.message_id = None;
            state.text.clear();

            // Send thinking state reset
            let thinking = ThinkingState::default();
            let _ = event_tx.send(SessionEvent::Thinking(thinking)).await;

            debug!(
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                stop_reason = ?stop_reason,
                "Message completed"
            );
        }

        AgentEvent::Error { error } => {
            let _ = event_tx.send(SessionEvent::Error(error)).await;
        }

        AgentEvent::Done {
            iterations,
            tool_calls,
            total_usage,
            ..
        } => {
            let usage = TokenUsage {
                input_tokens: total_usage.input_tokens,
                output_tokens: total_usage.output_tokens,
                cache_read_tokens: total_usage.cache_read_tokens,
                total_cost: None,
            };
            let _ = event_tx.send(SessionEvent::SessionEnd(usage)).await;

            info!(
                iterations = iterations,
                tool_calls = tool_calls,
                "Agent loop completed"
            );
        }
        AgentEvent::SubAgentSpawn { task } => {
            let msg = Message::system(format!("Spawning sub-agent for task: {}", task));
            let _ = event_tx.send(SessionEvent::Message(msg)).await;
        }
        AgentEvent::StateChange {
            old_state,
            new_state,
        } => {
            debug!(?old_state, ?new_state, "Agent state changed");
            // Optionally forward to UI in a later phase
        }
        AgentEvent::Cleanup { pruned_count } => {
            info!(pruned = pruned_count, "Resource cleanup performed");
            let msg = Message::system(format!(
                "Resource cleanup: pruned {} completed agents",
                pruned_count
            ));
            let _ = event_tx.send(SessionEvent::Message(msg)).await;
        }
        AgentEvent::Start { session_id } => {
            info!(id = %session_id, "Agent session started");
        }
        AgentEvent::IterationEnd { iteration, usage } => {
            debug!(
                iteration = iteration,
                input = usage.input_tokens,
                output = usage.output_tokens,
                "Iteration completed"
            );
        }
        AgentEvent::WaitingApproval { id, name } => {
            let _ = event_tx
                .send(SessionEvent::WaitingApproval { id, name })
                .await;
        }
        AgentEvent::Finished { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert!(!config.session_id.is_empty());
    }
}
