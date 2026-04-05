//! Main Agent Event Handling
//!
//! Handles agent events for the active/main workspace, including
//! streaming text, tool execution state, token usage tracking,
//! sub-agent spawning, and permission requests.

use std::time::Instant;

use anyhow::Result;
use tracing::{error, info};

use super::state_fields::apply_event_to_state_fields;
use crate::app::{App, ToolExecutionState};
use crate::event::Event;
use crate::ipc::TokenUsage;

impl App {
    /// Handle an agent event
    pub async fn handle_agent_event(&mut self, event: crate::agent::AgentEvent) -> Result<()> {
        // Stream out to file if enabled
        if let crate::agent::AgentEvent::Text { ref text } = event {
            if let Some(path) = &self.session.stream_out {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    use std::io::Write;
                    let _ = write!(f, "{}", text);
                }
            }
        }

        // 1. Move fields into locals to allow mutable borrowing of self
        let mut messages = std::mem::take(&mut self.session.messages);
        let mut thinking = std::mem::take(&mut self.session.thinking);
        let mut streaming_message = std::mem::take(&mut self.agents.streaming_message);

        apply_event_to_state_fields(
            event.clone(),
            &mut messages,
            &mut thinking,
            &mut streaming_message,
        )?;

        // Restore the fields
        self.session.messages = messages;
        self.session.thinking = thinking;
        self.agents.streaming_message = streaming_message;

        // 2. Handle side effects that require broader App access
        match event {
            crate::agent::AgentEvent::ToolStart { id, name } => {
                if !self.tools.executing_tools.iter().any(|t| t.id == id) {
                    self.tools.executing_tools.push(ToolExecutionState {
                        id,
                        name,
                        input: serde_json::Value::Null,
                        start_time: Instant::now(),
                        is_executing: true,
                        output: None,
                        is_error: false,
                        elapsed_ms: 0,
                    });
                }
            }
            crate::agent::AgentEvent::ToolEnd {
                id,
                result,
                is_error,
                elapsed_ms,
                ..
            } => {
                self.complete_tool_call(&id, result, is_error, elapsed_ms);
            }
            crate::agent::AgentEvent::MessageEnd { usage, .. } => {
                let model = self
                    .model
                    .clone()
                    .unwrap_or_else(|| self.config.model.clone());
                let token_usage = TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    total_cost: None,
                };
                let cost = crate::agent::cost::calculate_cost(&token_usage, &model);
                self.session.session_cost = cost;

                let mut final_usage = token_usage;
                final_usage.total_cost = Some(self.session.session_cost);
                self.session.token_usage = final_usage;
                self.session.thinking_start = None;
            }
            crate::agent::AgentEvent::Error { .. } => {
                self.session.thinking_start = None;
                self.check_queue()?;
            }
            crate::agent::AgentEvent::Done {
                iterations,
                total_usage,
                ..
            } => {
                info!("Agent completed: {} iterations", iterations);
                let model = self
                    .model
                    .clone()
                    .unwrap_or_else(|| self.config.model.clone());
                let token_usage = TokenUsage {
                    input_tokens: total_usage.input_tokens,
                    output_tokens: total_usage.output_tokens,
                    cache_read_tokens: total_usage.cache_read_tokens,
                    total_cost: None,
                };
                let cost = crate::agent::cost::calculate_cost(&token_usage, &model);
                self.session.session_cost = cost;

                let mut final_usage = token_usage;
                final_usage.total_cost = Some(self.session.session_cost);
                self.session.token_usage = final_usage;
                self.session.thinking_start = None;
                self.check_queue()?;

                // Trigger save
                if self.agents.agent_loop.is_some() {
                    if let Some(tx) = &self.event_tx {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(Event::SaveSession).await;
                        });
                    }
                }
            }
            crate::agent::AgentEvent::SubAgentSpawn { task } => {
                info!("Agent requested sub-agent spawn for: {}", task);
                self.add_system_message(&format!("Agent spawning sub-task: '{}'", task));

                let config = crate::agent::AgentConfig {
                    model: self
                        .model
                        .clone()
                        .unwrap_or_else(|| self.config.model.clone()),
                    system_prompt: crate::agent::prompt::build_system_prompt_with_options(
                        &self.cwd.as_deref().unwrap_or("."),
                        Some(&crate::agent::prompt::Role::Executor),
                        false,
                    ),
                    parent_session_id: self.current_parent_session_id(),
                    allow_parallel_spawn: false,
                    plan_mode: self.ui.plan_mode,
                    skip_compaction: true,
                    ..Default::default()
                };

                if let Some(provider) = &self.provider {
                    match self
                        .subagents
                        .spawn(
                            task.clone(),
                            config,
                            provider.clone(),
                            self.tools.tool_coordinator.clone(),
                            None,
                            self.agents.parallel_agents_enabled,
                        )
                        .await
                    {
                        Ok((id, rx)) => {
                            self.add_inline_agent(id.clone(), task.clone());
                            self.add_system_message(&format!(
                                "Sub-agent spawned successfully! (ID: {})",
                                &id[..8]
                            ));
                            self.spawn_agent_forwarder(id, rx);
                        }
                        Err(e) => {
                            error!("Failed to spawn sub-agent: {}", e);
                            self.add_system_message(&format!("Error spawning sub-agent: {}", e));
                        }
                    }
                } else {
                    self.add_system_message("Cannot spawn sub-agent: No provider available.");
                }
            }
            crate::agent::AgentEvent::WaitingApproval { id, name: _ } => {
                if let Some(agent) = &self.agents.agent_loop {
                    let guard = agent.guard.clone();
                    if let Some(guard) = guard {
                        let id = id.clone();
                        if let Some(req) = guard.get_pending_request(&id).await {
                            self.session.permission_request = Some(req);
                            self.add_notification(
                                "Command waiting for approval",
                                crate::app::state::NotificationType::Info,
                            );
                        }
                    }
                }
            }
            // Remaining variants that don't need special processing
            crate::agent::AgentEvent::Start { .. }
            | crate::agent::AgentEvent::Thinking { .. }
            | crate::agent::AgentEvent::Text { .. }
            | crate::agent::AgentEvent::ToolInput { .. }
            | crate::agent::AgentEvent::IterationEnd { .. }
            | crate::agent::AgentEvent::StateChange { .. }
            | crate::agent::AgentEvent::Cleanup { .. }
            | crate::agent::AgentEvent::Finished { .. } => {}
        }

        Ok(())
    }
}
