//! Sub-agent spawning logic (regular and typed)

use super::types::{SubAgentHandle, SubAgentStatus};
use crate::agent::prompt::Role as PromptRole;
use crate::agent::specialists::AgentType;
use crate::agent::{AgentConfig, AgentLoop};
use crate::tools::AgentRole;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

impl super::SubAgentManager {
    /// Spawn a new sub-agent
    ///
    /// If `inline` is true, the agent runs in-process without worktree isolation.
    /// If `inline` is false, creates an isolated worktree (for --vex mode).
    pub async fn spawn(
        &self,
        task: String,
        mut config: AgentConfig,
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<crate::agent::ToolCoordinator>,
        role: Option<AgentRole>,
        inline: bool,
    ) -> Result<(String, broadcast::Receiver<crate::agent::AgentEvent>)> {
        let id = Uuid::new_v4().to_string();
        let start_time = Utc::now();
        let last_activity = start_time;

        // Set role for tool access control (default to Executor for sub-agents)
        config.role = role.unwrap_or(AgentRole::Executor);

        // Track if we created a worktree (for cleanup later)
        let mut worktree_path = None;

        // Only create worktree if NOT inline mode
        if !inline {
            worktree_path = self.create_worktree(&id, &mut config);
        } else {
            tracing::info!("Spawning inline sub-agent {} (no worktree)", id);
        }

        let parent_session_id = config.parent_session_id.clone();

        // Ensure delegated agents stay delegated.
        config.is_subagent = true;
        config.delegation_depth = config.delegation_depth.saturating_add(1);
        config.allow_parallel_spawn = false;
        config.session_id = id.clone();
        if config.system_prompt.is_empty() {
            config.system_prompt = crate::agent::prompt::build_system_prompt_with_options(
                &config.working_dir,
                Some(&PromptRole::Executor),
                false,
            );
        }

        // Register mailbox for inter-agent messaging
        super::mailbox::register_agent(&id);

        let handle = SubAgentHandle {
            id: id.clone(),
            task: task.clone(),
            status: SubAgentStatus::Running,
            start_time,
            end_time: None,
            iterations: 0,
            last_activity,
            error: None,
            result: None,
            parent_session_id,
            worktree_path,
            current_action: None,
        };
        {
            let mut agents = self.agents.write().await;
            agents.insert(id.clone(), handle.clone());
        }

        self.persist_handle_to_db(&id, &task, &handle, provider.name(), &config.model);

        let (agent_loop, mut _events) = AgentLoop::with_events(
            provider, tools, None, // Sub-agents are autonomous for now
            config,
        );

        let agent_loop = Arc::new(agent_loop);
        let loop_id = id.clone();
        let agents_ref = self.agents.clone();
        let sub = agent_loop.subscribe();
        let agent_loop_for_spawn = agent_loop.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        tokio::spawn(async move {
            agent_loop_for_spawn.add_user_message(&task).await;

            let mut events = agent_loop_for_spawn.subscribe();
            let loop_id_inner = loop_id.clone();
            let agents_inner = agents_ref.clone();
            let broadcast_tx_inner = broadcast_tx.clone();
            let agent_loop_inner = agent_loop_for_spawn.clone();
            tokio::spawn(async move {
                while let Ok(event) = events.recv().await {
                    // Forward event to subagent broadcast channel (for UI updates)
                    let _ = broadcast_tx_inner.send(event.clone());

                    if let Some(agent) = agents_inner.write().await.get_mut(&loop_id_inner) {
                        agent.last_activity = Utc::now();
                        agent.iterations = agent_loop_inner.state_tracker.get_iterations().await;

                        match event {
                            crate::agent::AgentEvent::Text { text } => {
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
                            _ => {}
                        }
                    }
                }
            });

            let result = agent_loop_for_spawn.run().await;

            let task_completed = result.as_ref().map(|r| r.task_completed).unwrap_or(false);
            let had_error = result.is_err();

            if let Some(agent) = agents_ref.write().await.get_mut(&loop_id) {
                if had_error {
                    agent.status = SubAgentStatus::Failed;
                    agent.error = result.err().map(|e| e.to_string());
                } else if task_completed {
                    agent.status = SubAgentStatus::Completed;
                } else {
                    agent.status = SubAgentStatus::Ended;
                }
                agent.end_time = Some(Utc::now());
            }

            // Unregister mailbox when agent finishes
            super::mailbox::unregister_agent(&loop_id);

            if task_completed {
                tracing::info!("Sub-agent {} formally completed via complete_task", loop_id);
            } else {
                tracing::info!(
                    "Sub-agent {} ended (stopped without calling complete_task)",
                    loop_id
                );
            }
        });

        Ok((id, sub))
    }

    /// Spawn a new sub-agent with a specific specialization
    ///
    /// This is the preferred method for spawning specialized agents.
    /// The agent will receive domain-specific system prompts.
    pub async fn spawn_with_type(
        &self,
        task: String,
        mut config: AgentConfig,
        provider: Arc<dyn crate::providers::Provider>,
        tools: Arc<crate::agent::ToolCoordinator>,
        role: Option<AgentRole>,
        inline: bool,
        agent_type: AgentType,
    ) -> Result<(String, broadcast::Receiver<crate::agent::AgentEvent>)> {
        let profile = agent_type.profile();

        if role.is_none() {
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

        self.spawn(task, config, provider, tools, role, inline)
            .await
    }

    /// Create a git worktree for sub-agent isolation.
    ///
    /// Returns Some(worktree_path) if created, None otherwise.
    pub(crate) fn create_worktree(&self, id: &str, config: &mut AgentConfig) -> Option<String> {
        let is_git = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .current_dir(&config.working_dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !is_git {
            return None;
        }

        let worktree_dir = std::path::PathBuf::from(format!("/tmp/d3vx_worktrees/{}", id));
        if let Err(e) = std::fs::create_dir_all(worktree_dir.parent().unwrap()) {
            tracing::warn!("Failed to create worktree parent dir: {}", e);
            return None;
        }

        let branch_name = format!("d3vx-task-{}", &id[..8]);
        let output = std::process::Command::new("git")
            .args(&[
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_dir.to_str().unwrap(),
                "HEAD",
            ])
            .current_dir(&config.working_dir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                tracing::info!(
                    "Created isolated git worktree for sub-agent {} at {:?}",
                    id,
                    worktree_dir
                );
                config.working_dir = worktree_dir.to_string_lossy().to_string();
                let path = config.working_dir.clone();
                config.system_prompt = crate::agent::prompt::build_system_prompt_with_options(
                    &config.working_dir,
                    Some(&PromptRole::Executor),
                    false,
                );
                Some(path)
            }
            Ok(o) => {
                tracing::warn!(
                    "Failed to create git worktree (inline mode fallback): {}",
                    String::from_utf8_lossy(&o.stderr)
                );
                None
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to execute git worktree command, running inline: {}",
                    e
                );
                None
            }
        }
    }

    /// Persist a sub-agent handle to the database.
    pub(crate) fn persist_handle_to_db(
        &self,
        id: &str,
        task: &str,
        handle: &SubAgentHandle,
        provider_name: &str,
        model: &str,
    ) {
        if let Some(db_handle) = &self.db {
            let db = db_handle.lock();
            let store = crate::store::session::SessionStore::new(&db);
            let metadata = serde_json::to_string(handle).unwrap_or_else(|_| "{}".to_string());

            let _ = store.create(crate::store::session::NewSession {
                id: Some(id.to_string()),
                task_id: None,
                provider: provider_name.to_string(),
                model: model.to_string(),
                messages: None,
                token_count: None,
                summary: Some(task.to_string()),
                project_path: None,
                parent_session_id: handle.parent_session_id.clone(),
                metadata: Some(metadata),
                state: None,
            });
        }
    }
}
