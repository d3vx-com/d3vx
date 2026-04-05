//! Workspace Agent Event Handling
//!
//! Handles agent events routed to specific workspaces, including
//! inline agent updates, parallel batch completion, and background
//! workspace state synchronization.

use anyhow::Result;

use super::state_fields::apply_event_to_state_fields;
use crate::app::{App, InlineAgentUpdate};
use crate::ipc::ThinkingState;

impl App {
    /// Handle an agent event for a specific workspace
    pub async fn handle_workspace_agent_event(
        &mut self,
        workspace_id: &str,
        event: crate::agent::AgentEvent,
    ) -> Result<()> {
        // Update inline agent with ongoing events
        match &event {
            crate::agent::AgentEvent::Thinking { text } => {
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Action(text.clone()));

                // Merge thinking deltas
                let merged = if let Some(agent) = self
                    .agents
                    .inline_agents
                    .iter_mut()
                    .find(|a| a.id == workspace_id)
                {
                    if let Some(last_msg) = agent.messages.last_mut() {
                        if last_msg.line_type == crate::app::state::AgentLineType::Thinking {
                            last_msg.content.push_str(text);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !merged {
                    let msg = crate::app::state::AgentMessageLine {
                        content: text.clone(),
                        line_type: crate::app::state::AgentLineType::Thinking,
                        timestamp: std::time::Instant::now(),
                    };
                    self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
                }
            }
            crate::agent::AgentEvent::ToolStart { id: _, name } => {
                let msg = crate::app::state::AgentMessageLine {
                    content: format!("[Tool: {}]", name),
                    line_type: crate::app::state::AgentLineType::ToolCall,
                    timestamp: std::time::Instant::now(),
                };
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Tool(name.clone()));
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
            }
            crate::agent::AgentEvent::ToolInput { json } => {
                let content = format!("Input: {}", json);
                let msg = crate::app::state::AgentMessageLine {
                    content: content.clone(),
                    line_type: crate::app::state::AgentLineType::ToolCall,
                    timestamp: std::time::Instant::now(),
                };
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Action(content));
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
            }
            crate::agent::AgentEvent::ToolEnd { result, .. } => {
                let truncated = if result.len() > 200 {
                    format!("{}..", &result[..198])
                } else {
                    result.clone()
                };
                let msg = crate::app::state::AgentMessageLine {
                    content: format!("Output: {}", truncated),
                    line_type: crate::app::state::AgentLineType::ToolOutput,
                    timestamp: std::time::Instant::now(),
                };
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Output(truncated));
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
            }
            crate::agent::AgentEvent::Text { text } => {
                // Merge text deltas
                let merged = if let Some(agent) = self
                    .agents
                    .inline_agents
                    .iter_mut()
                    .find(|a| a.id == workspace_id)
                {
                    if let Some(last_msg) = agent.messages.last_mut() {
                        if last_msg.line_type == crate::app::state::AgentLineType::Text {
                            last_msg.content.push_str(text);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !merged {
                    let msg = crate::app::state::AgentMessageLine {
                        content: text.clone(),
                        line_type: crate::app::state::AgentLineType::Text,
                        timestamp: std::time::Instant::now(),
                    };
                    self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
                }
            }
            crate::agent::AgentEvent::Error { error } => {
                let msg = crate::app::state::AgentMessageLine {
                    content: format!("Error: {}", error),
                    line_type: crate::app::state::AgentLineType::Text,
                    timestamp: std::time::Instant::now(),
                };
                // Don't set Failed status here -- the agent may still be
                // retrying.  Terminal states (Failed / Ended) are determined
                // by the Finished handler which runs after the agent loop has
                // truly exited.
                self.update_inline_agent(workspace_id, InlineAgentUpdate::Message(msg));
            }
            crate::agent::AgentEvent::Finished { .. } => {
                // Finished is emitted after Done (on success) or without Done
                // (fatal error).  Only update the status if the agent is
                // still Running, meaning Done was never processed.
                let is_still_running = self
                    .agents
                    .inline_agents
                    .iter()
                    .find(|a| a.id == workspace_id)
                    .map(|a| a.status == crate::app::state::InlineAgentStatus::Running)
                    .unwrap_or(false);

                if is_still_running {
                    let final_status = if let Some(handle) = self.subagents.get(workspace_id).await
                    {
                        if handle.status == crate::agent::SubAgentStatus::Failed {
                            crate::app::state::InlineAgentStatus::Failed
                        } else {
                            crate::app::state::InlineAgentStatus::Ended
                        }
                    } else {
                        crate::app::state::InlineAgentStatus::Ended
                    };
                    self.update_inline_agent(workspace_id, InlineAgentUpdate::Status(final_status));
                }
            }
            _ => {}
        }

        // Handle sub-agent specific completion
        if let crate::agent::AgentEvent::Done { final_text, .. } = &event {
            self.handle_workspace_agent_done(workspace_id, final_text)
                .await?;
        }

        let current_ws_idx = self.workspace_selected_index;
        if current_ws_idx >= self.workspaces.len() {
            return Ok(());
        }
        let current_id = self
            .workspaces
            .get(current_ws_idx)
            .map(|ws| ws.id.clone())
            .unwrap_or_default();

        if workspace_id == current_id {
            self.handle_agent_event(event).await
        } else {
            let state = self
                .workspace_states
                .entry(workspace_id.to_string())
                .or_insert_with(|| crate::app::WorkspaceState {
                    messages: Vec::new(),
                    session_id: Some(workspace_id.to_string()),
                    streaming_message: String::new(),
                    thinking: ThinkingState::default(),
                });
            apply_event_to_state_fields(
                event.clone(),
                &mut state.messages,
                &mut state.thinking,
                &mut state.streaming_message,
            )?;

            // If a background workspace needs approval, show the popup
            if let crate::agent::AgentEvent::WaitingApproval { id, .. } = event {
                if let Some(agent) = self.agents.workspace_agents.get(workspace_id) {
                    let guard = agent.guard.clone();
                    if let Some(guard) = guard {
                        if let Some(req) = guard.get_pending_request(&id).await {
                            self.session.permission_request = Some(req);
                            self.add_notification(
                                format!("Background task in '{}' needs approval", workspace_id),
                                crate::app::state::NotificationType::Info,
                            );
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
