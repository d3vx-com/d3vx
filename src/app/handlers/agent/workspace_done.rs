//! Workspace Agent Done Handling
//!
//! Handles the AgentEvent::Done path for workspace agents, including
//! sub-agent completion, parallel batch updates, and parent feedback.

use anyhow::Result;

use crate::app::{App, InlineAgentUpdate, ParallelChildStatus};

impl App {
    /// Process AgentEvent::Done for a workspace agent
    pub(super) async fn handle_workspace_agent_done(
        &mut self,
        workspace_id: &str,
        final_text: &str,
    ) -> Result<()> {
        // Always update inline agent status to Completed regardless of
        // whether the subagent handle is found.
        self.update_inline_agent(
            workspace_id,
            InlineAgentUpdate::Status(crate::app::state::InlineAgentStatus::Completed),
        );

        let truncated = if final_text.len() > 100 {
            format!("{}..", &final_text[..98])
        } else {
            final_text.to_string()
        };
        self.update_inline_agent(workspace_id, InlineAgentUpdate::Output(truncated));

        if let Some(agent) = self.subagents.get(workspace_id).await {
            let parent_id = agent.parent_session_id.clone();
            let task_description = agent.task.clone();
            let result_text = final_text.to_string();

            self.subagents
                .update_status(
                    workspace_id,
                    crate::agent::SubAgentStatus::Completed,
                    Some(result_text.clone()),
                )
                .await;

            if let Some(batch_id) = self.update_parallel_batch_child(
                workspace_id,
                ParallelChildStatus::Completed,
                Some(result_text.clone()),
            ) {
                self.finalize_parallel_batch(&batch_id).await;
            }

            // Decrement running agent count if this was a parallel agent
            if self.agents.running_parallel_agents > 0 {
                self.agents.running_parallel_agents -= 1;

                if let Some(ref provider) = self.provider {
                    self.spawn_ready_parallel_tasks(provider.clone()).await;
                }

                // When all parallel agents finish, just reset the counter.
                // finalize_parallel_batch (called above) already handles
                // message injection and RunSynthesis — avoid duplicates.
                if self.agents.running_parallel_agents == 0
                    && self.agents.pending_agent_queue.is_empty()
                    && self.agents.active_parallel_batches > 0
                {
                    self.agents.active_parallel_batches = 0;
                }
            }

            self.add_system_message(&format!("Sub-agent completed its task: {}", workspace_id));
            self.add_notification(
                format!(
                    "{} Sub-agent finished: {}",
                    crate::ui::symbols::CHECK,
                    final_text.chars().take(30).collect::<String>()
                ),
                crate::app::state::NotificationType::Success,
            );

            // FEEDBACK LOOP: Notify parent workspace if it exists
            if let Some(pid) = parent_id {
                let feedback = format!(
                    "### Sub-agent Analysis Complete\n**Sub-agent ID:** `{}`\n**Task:** {}\n\n**Result:**\n{}",
                    &workspace_id[..workspace_id.len().min(8)],
                    task_description,
                    result_text
                );

                let _state = self.workspace_states.entry(pid.clone()).or_insert_with(|| {
                    crate::app::WorkspaceState {
                        messages: Vec::new(),
                        session_id: Some(pid.clone()),
                        streaming_message: String::new(),
                        thinking: crate::ipc::ThinkingState::default(),
                    }
                });

                self.add_system_message(&format!(
                    "Analysis results from sub-agent {} added to parent session context.",
                    &workspace_id[..workspace_id.len().min(8)]
                ));

                // Inject into parent agent loop if it exists (async-safe).
                // The parent agent loop is blocked on spawn_parallel tool's
                // oneshot channel — when finalize_parallel_batch sends the
                // response, the loop naturally processes the injected message.
                // No need for RunSynthesis here.
                if let Some(active_loop) = &self.agents.agent_loop {
                    let active_loop = active_loop.clone();
                    let pid_clone = pid.clone();
                    let feedback_clone = feedback.clone();

                    tokio::spawn(async move {
                        let config = active_loop.config.read().await;
                        let active_session_id = config.session_id.clone();
                        drop(config);

                        if active_session_id == pid_clone {
                            let msg = crate::providers::Message {
                                role: crate::providers::Role::User,
                                content: crate::providers::MessageContent::Text(format!(
                                    "[SYSTEM] Sub-agent analysis complete.\n{}",
                                    feedback_clone
                                )),
                            };
                            active_loop.conversation.write().await.add_message(msg);
                        }
                    });
                }
            }
        }

        Ok(())
    }
}
