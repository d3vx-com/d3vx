//! Agent Start Bookkeeping and Inbox Handling
//!
//! Handles common and regular agent-started logic (status tracking,
//! persistence) and inter-agent inbox message delivery.

use anyhow::Result;
use tracing::info;

use crate::app::{App, ParallelChildStatus};

impl App {
    /// Handle common logic when a regular agent starts
    pub(super) async fn handle_regular_agent_started(
        &mut self,
        batch_id: &str,
        task: &crate::tools::SpawnTask,
        id: &str,
    ) {
        self.handle_common_agent_started(batch_id, task, id);
        tracing::info!(
            "Agent started: [{}] {}",
            task.agent_type.display_name(),
            task.description
        );
    }

    /// Common logic for when an agent starts (inline or regular)
    pub(super) fn handle_common_agent_started(
        &mut self,
        batch_id: &str,
        task: &crate::tools::SpawnTask,
        id: &str,
    ) {
        let mut child_task_id = None;
        if let Some(batch) = self.agents.parallel_batches.get_mut(batch_id) {
            if let Some(child) = batch.children.iter_mut().find(|child| {
                child.key == task.key
                    && child.agent_id.is_none()
                    && child.status == ParallelChildStatus::Pending
            }) {
                child.agent_id = Some(id.to_string());
                child.status = ParallelChildStatus::Running;
                child_task_id = child.task_id.clone();
            }
        }
        if let Some(task_id) = child_task_id {
            if let Some(db_handle) = &self.db {
                let db = db_handle.lock();
                let task_store = crate::store::task::TaskStore::from_connection(db.connection());
                let existing = task_store
                    .get(&task_id)
                    .ok()
                    .flatten()
                    .map(|task| task.metadata)
                    .unwrap_or_else(|| "{}".to_string());
                let merged = Self::merge_task_metadata(
                    &existing,
                    serde_json::json!({
                        "orchestration_node": {
                            "agent_id": id,
                            "status": "Running",
                        }
                    }),
                );
                let _ = task_store.update(
                    &task_id,
                    crate::store::task::TaskUpdate {
                        state: Some(crate::store::task::TaskState::Spawning),
                        metadata: Some(merged),
                        ..Default::default()
                    },
                );
            }
        }
        self.persist_parallel_batch_snapshot(batch_id);
        self.add_inline_agent(
            id.to_string(),
            format!("[{}] {}", task.agent_type.display_name(), task.description),
        );
    }

    /// Handle an inbox message event
    pub async fn handle_inbox_message(
        &mut self,
        to_agent: &str,
        from_agent: &str,
        message: &str,
    ) -> Result<()> {
        info!("Inbox message from {} to {}", from_agent, to_agent);

        let is_main_agent = if to_agent == "tech_lead" {
            true
        } else if let Some(agent) = &self.agents.agent_loop {
            let config = agent.config.read().await;
            config.session_id == to_agent
        } else {
            false
        };

        if is_main_agent {
            self.add_notification(
                format!("New message from agent '{}'", from_agent),
                crate::app::state::NotificationType::Info,
            );

            let display_msg = format!("**[INBOX] Message from {}:**\n{}", from_agent, message);
            self.add_system_message(&display_msg);

            // Inject into Tech Lead's conversation history
            if let Some(agent) = &self.agents.agent_loop {
                let agent_clone = agent.clone();
                let history_msg = format!("[INCOMING MESSAGE FROM {}]: {}", from_agent, message);
                tokio::spawn(async move {
                    agent_clone.add_user_message(history_msg).await;
                });
            }
        } else {
            self.add_notification(
                format!("Message relayed: {} -> {}", from_agent, to_agent),
                crate::app::state::NotificationType::Info,
            );
        }

        Ok(())
    }
}
