//! Spawn Parallel Event and Single Agent Spawning
//!
//! Handles the SpawnParallel event and individual agent spawning
//! from task queues with concurrency limits.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tracing::error;

use crate::agent::specialists::AgentType;
use crate::app::handlers::agent::coordination::BatchCoordination;
use crate::app::{App, ParallelBatchState, ParallelChildStatus, ParallelChildTask};
use crate::tools::SpawnParallelEvent;

impl App {
    /// Handle a SpawnParallel event - spawn multiple agents in parallel with concurrency limit
    pub async fn handle_spawn_parallel_event(&mut self, event: SpawnParallelEvent) -> Result<()> {
        tracing::info!(
            "handle_spawn_parallel_event: received event with {} tasks",
            event.tasks.len()
        );

        const MAX_CONCURRENT_AGENTS: usize = 5;
        self.agents.active_parallel_batches += 1;
        self.agents.parallel_batches.insert(
            event.batch_id.clone(),
            ParallelBatchState {
                id: event.batch_id.clone(),
                parent_session_id: event.parent_session_id.clone(),
                reasoning: event.reasoning.clone(),
                select_best: event.select_best,
                selection_criteria: event.selection_criteria.clone(),
                selected_child_key: None,
                selection_reasoning: None,
                started_at: Instant::now(),
                completed_at: None,
                children: event
                    .tasks
                    .iter()
                    .map(|task| ParallelChildTask {
                        key: task.key.clone(),
                        description: task.description.clone(),
                        task: task.task.clone(),
                        agent_type: task.agent_type.display_name().to_string(),
                        specialist_role: task.agent_type.specialist_role_label().to_string(),
                        depends_on: task.depends_on.clone(),
                        ownership: task.ownership.clone(),
                        task_id: None,
                        agent_id: None,
                        status: ParallelChildStatus::Pending,
                        result: None,
                        evaluation: None,
                        progress: 0,
                        blocked: false,
                        blocker_reason: None,
                        messages_sent: 0,
                        messages_received: 0,
                    })
                    .collect(),
                coordination: BatchCoordination::new(),
                response_tx: std::sync::Arc::new(std::sync::Mutex::new(Some(event.response_tx))),
            },
        );

        if let Some(db_handle) = &self.db {
            let db = db_handle.lock();
            let task_store = crate::store::task::TaskStore::from_connection(db.connection());
            let parent_task_id = self.current_parent_task_id();
            let project_path = self.cwd.clone();

            if let Some(batch) = self.agents.parallel_batches.get_mut(&event.batch_id) {
                for child in &mut batch.children {
                    let agent_type = event
                        .tasks
                        .iter()
                        .find(|task| task.key == child.key)
                        .map(|task| task.agent_type)
                        .unwrap_or(AgentType::General);
                    if let Ok(task) = task_store.create(crate::store::task::NewTask {
                        id: None,
                        title: format!("{}: {}", child.specialist_role, child.description),
                        description: Some(child.task.clone()),
                        state: Some(crate::store::task::TaskState::Queued),
                        priority: Some(0),
                        batch_id: Some(event.batch_id.clone()),
                        max_retries: None,
                        depends_on: Some(child.depends_on.clone()),
                        metadata: Some(serde_json::json!({
                            "orchestration_node": {
                                "batch_id": event.batch_id,
                                "key": child.key,
                                "specialist_role": child.specialist_role,
                                "ownership": child.ownership,
                                "reasoning": event.reasoning,
                            }
                        })),
                        project_path: project_path.clone(),
                        agent_role: Some(Self::map_agent_type_to_store_role(agent_type)),
                        execution_mode: Some(crate::store::task::ExecutionMode::Direct),
                        repo_root: self.cwd.clone(),
                        task_scope_path: child.ownership.clone(),
                        scope_mode: None,
                        parent_task_id: parent_task_id.clone(),
                    }) {
                        child.task_id = Some(task.id);
                    }
                }
            }
        }
        self.persist_parallel_batch_snapshot(&event.batch_id);

        let provider = self.provider.clone();
        if provider.is_none() {
            self.add_system_message("Cannot spawn agents: No LLM provider available.");
            return Ok(());
        }
        let provider = provider.unwrap();

        // Add all tasks to the queue; dependency-aware scheduling decides
        // which children can start immediately and which remain pending.
        for task in event.tasks {
            self.agents
                .pending_agent_queue
                .push((event.batch_id.clone(), task));
        }
        self.spawn_ready_parallel_tasks(provider.clone()).await;

        // Show summary
        self.add_system_message(&format!(
            "Coordinated multi-agent batch {} started: {}",
            &event.batch_id[..event.batch_id.len().min(8)],
            event.reasoning
        ));
        if self.agents.pending_agent_queue.is_empty() {
            self.add_system_message(&format!(
                "{} agents running (max {} concurrent)",
                self.agents.running_parallel_agents, MAX_CONCURRENT_AGENTS
            ));
        } else {
            self.add_system_message(&format!(
                "{} agents started, {} queued (max {} concurrent)",
                self.agents.running_parallel_agents,
                self.agents.pending_agent_queue.len(),
                MAX_CONCURRENT_AGENTS
            ));
        }

        Ok(())
    }

    /// Spawn a single agent from a task
    pub(super) async fn spawn_single_agent(
        &mut self,
        batch_id: &str,
        task: &crate::tools::SpawnTask,
        provider: Arc<dyn crate::providers::Provider>,
    ) {
        let specialist_profile = task.agent_type.profile();
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
            role: specialist_profile.recommended_role,
            skip_compaction: true,
            ..Default::default()
        };

        match self
            .subagents
            .spawn_with_type(
                task.task.clone(),
                config,
                provider,
                self.tools.tool_coordinator.clone(),
                Some(specialist_profile.recommended_role),
                self.agents.parallel_agents_enabled,
                task.agent_type,
            )
            .await
        {
            Ok((id, rx)) => {
                self.handle_regular_agent_started(batch_id, task, &id).await;
                self.spawn_agent_forwarder(id, rx);
            }
            Err(e) => {
                error!("Failed to spawn agent: {}", e);
                self.add_system_message(&format!("Failed to spawn agent: {}", e));
            }
        }
    }

    /// Spawn the next agent from the queue (called when an agent completes)
    pub fn spawn_next_from_queue(&mut self) {
        if self.agents.pending_agent_queue.is_empty() {
            return;
        }

        tracing::info!(
            "spawn_next_from_queue called, {} agents in queue",
            self.agents.pending_agent_queue.len()
        );
    }
}
