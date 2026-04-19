//! Inline sub-agent spawning
//!
//! Handles spawning inline agents that share the parent's conversation.

use super::types::{InlineCallback, SubAgentHandle, SubAgentStatus};
use crate::agent::specialists::AgentType;
use crate::agent::{AgentConfig, AgentLoop};
use crate::tools::AgentRole;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

impl super::SubAgentManager {
    /// Spawn an inline agent that shares the parent's conversation.
    ///
    /// Unlike regular sub-agents, inline agents:
    /// - Share the parent's conversation directly
    /// - Stream output via callback instead of events
    /// - Don't create separate message history
    /// - Run faster with less overhead
    pub async fn spawn_inline(
        &self,
        task: String,
        mut config: AgentConfig,
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<crate::agent::ToolCoordinator>,
        conversation: Arc<RwLock<crate::agent::Conversation>>,
        role: Option<AgentRole>,
        agent_type: AgentType,
        on_output: InlineCallback,
    ) -> Result<(String, broadcast::Receiver<crate::agent::AgentEvent>)> {
        let id = Uuid::new_v4().to_string();
        let profile = agent_type.profile();

        if let Some(r) = role {
            config.role = r;
        } else {
            config.role = profile.recommended_role;
        }

        if !profile.system_prompt.is_empty() {
            config.system_prompt = format!(
                "{}\n\n{}",
                config.system_prompt.trim_end(),
                profile.system_prompt
            );
        }

        if let Some(project_context) = agent_type.resolve_project_context(&config.working_dir) {
            config.system_prompt =
                format!("{}\n\n{}", config.system_prompt.trim_end(), project_context);
        }

        if !profile.review_focus.is_empty() {
            config.system_prompt = format!(
                "{}\n\nReview focus for this specialist:\n- {}",
                config.system_prompt.trim_end(),
                profile.review_focus.join("\n- ")
            );
        }

        config.skip_compaction = true;

        let agents = self.agents.clone();
        let broadcast_tx = self.broadcast_tx.clone();

        // Extract values needed for handle before consuming config
        let parent_session_id = config.parent_session_id.clone();

        // Create agent loop with shared conversation
        let agent_loop =
            AgentLoop::with_shared_conversation(provider, tools, None, config, conversation);

        let agent_loop = Arc::new(agent_loop);
        let loop_id = id.clone();

        // Register mailbox for inter-agent messaging
        super::mailbox::register_agent(&id);

        // Add handle to tracking
        let handle = SubAgentHandle {
            id: id.clone(),
            task: task.clone(),
            status: SubAgentStatus::Running,
            start_time: Utc::now(),
            end_time: None,
            iterations: 0,
            last_activity: Utc::now(),
            error: None,
            result: None,
            parent_session_id,
            worktree_path: None,
            current_action: None,
        };

        {
            let mut agents_guard = agents.write().await;
            agents_guard.insert(id.clone(), handle);
        }

        // Broadcast start event
        let _ = broadcast_tx.send(crate::agent::AgentEvent::Start {
            session_id: id.clone(),
        });

        // Clone for the spawned tasks
        let broadcast_tx_clone = broadcast_tx.clone();
        let on_output_clone = on_output.clone();
        let agents_clone = agents.clone();
        let loop_id_clone = loop_id.clone();

        // Spawn the agent task
        tokio::spawn(async move {
            // Add user message
            agent_loop.add_user_message(&task).await;

            // Process and stream output using a shared result
            let final_result = Arc::new(tokio::sync::Mutex::new(String::new()));

            // Subscribe to events
            let mut events = agent_loop.subscribe();
            let loop_id_inner = loop_id_clone.clone();
            let agents_inner = agents_clone.clone();
            let final_result_inner = final_result.clone();

            // Task to track iterations and forward events
            let agent_loop_track = agent_loop.clone();
            tokio::spawn(async move {
                while let Ok(event) = events.recv().await {
                    // Update agent tracking
                    if let Some(agent) = agents_inner.write().await.get_mut(&loop_id_inner) {
                        agent.last_activity = Utc::now();
                        agent.iterations = agent_loop_track.state_tracker.get_iterations().await;

                        match &event {
                            crate::agent::AgentEvent::Text { text } => {
                                on_output_clone(text.clone());
                                final_result_inner.lock().await.push_str(text);
                                final_result_inner.lock().await.push('\n');

                                let lines: Vec<&str> = text.lines().collect();
                                if let Some(first_line) = lines.first() {
                                    if !first_line.trim().is_empty() {
                                        agent.current_action = Some(first_line.to_string());
                                    }
                                }
                            }
                            crate::agent::AgentEvent::ToolStart { name, .. } => {
                                agent.current_action = Some(format!("Using tool: {}", name));
                            }
                            crate::agent::AgentEvent::ToolEnd {
                                name, result: _, ..
                            } => {
                                agent.current_action = Some(format!("Completed: {}", name));
                            }
                            _ => {}
                        }
                    }

                    // Forward event to subagent broadcast channel
                    let _ = broadcast_tx_clone.send(event);
                }
            });

            // Run the agent
            let result = agent_loop.run().await;

            // Update handle status based on whether task was formally completed.
            // See `spawn.rs` for rationale on treating safety-stops as failures.
            let result_text = final_result.lock().await.clone();
            let task_completed = result.as_ref().map(|r| r.task_completed).unwrap_or(false);
            let safety_stop_reason = result
                .as_ref()
                .ok()
                .and_then(|r| r.safety_stop_reason());
            let had_error = result.is_err();

            {
                let mut agents_guard = agents_clone.write().await;
                if let Some(agent) = agents_guard.get_mut(&loop_id_clone) {
                    if had_error {
                        agent.status = SubAgentStatus::Failed;
                        agent.error = result.err().map(|e| e.to_string());
                    } else if let Some(reason) = safety_stop_reason {
                        agent.status = SubAgentStatus::Failed;
                        agent.error = Some(format!("Agent stopped for safety: {reason}"));
                    } else if task_completed {
                        agent.status = SubAgentStatus::Completed;
                    } else {
                        agent.status = SubAgentStatus::Ended;
                    }
                    agent.end_time = Some(Utc::now());
                    agent.result = Some(result_text);
                }
            }

            // Unregister mailbox when inline agent finishes
            super::mailbox::unregister_agent(&loop_id_clone);

            if task_completed {
                tracing::info!(
                    "Inline agent {} formally completed via complete_task",
                    loop_id_clone
                );
            } else {
                tracing::info!(
                    "Inline agent {} ended (stopped without calling complete_task)",
                    loop_id_clone
                );
            }
        });

        // Return id and receiver for event forwarding
        let rx = broadcast_tx.subscribe();
        Ok((id, rx))
    }
}
