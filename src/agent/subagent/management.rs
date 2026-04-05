//! Sub-agent lifecycle management
//!
//! Handles listing, querying, status updates, and cleanup of sub-agents.

use super::types::{SubAgentHandle, SubAgentStatus};
use crate::config::types::CleanupConfig;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

impl super::SubAgentManager {
    pub async fn list(&self) -> Vec<SubAgentHandle> {
        self.agents.read().await.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<SubAgentHandle> {
        self.agents.read().await.get(id).cloned()
    }

    pub async fn update_status(&self, id: &str, status: SubAgentStatus, result: Option<String>) {
        if let Some(agent) = self.agents.write().await.get_mut(id) {
            agent.status = status.clone();
            agent.last_activity = Utc::now();
            if matches!(
                agent.status,
                SubAgentStatus::Completed | SubAgentStatus::Ended
            ) {
                agent.end_time = Some(Utc::now());
            }
            if result.is_some() {
                agent.result = result;
            }

            // Persist to database
            if let Some(db_handle) = &self.db {
                let db = db_handle.lock();
                let store = crate::store::session::SessionStore::new(&db);
                let metadata = serde_json::to_string(&agent).unwrap_or_else(|_| "{}".to_string());
                let _ = store.update(
                    id,
                    crate::store::session::SessionUpdate {
                        metadata: Some(metadata),
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Perform cleanup of old sub-agent handles and worktrees
    pub async fn cleanup(&self, config: &CleanupConfig) {
        let mut to_remove = Vec::new();
        let now = Utc::now();

        {
            let agents = self.agents.read().await;

            // Strategy 1: Remove by retention period
            for (id, handle) in agents.iter() {
                if handle.status != SubAgentStatus::Running {
                    if let Some(end_time) = handle.end_time {
                        let duration = now.signed_duration_since(end_time);
                        if duration.num_seconds() > config.retention_period_secs as i64 {
                            to_remove.push(id.clone());
                        }
                    } else {
                        // If no end_time but not running (e.g. failed early), use last_activity
                        let duration = now.signed_duration_since(handle.last_activity);
                        if duration.num_seconds() > config.retention_period_secs as i64 {
                            to_remove.push(id.clone());
                        }
                    }
                }
            }
        }

        // Strategy 2: Remove by max_retained if we have too many
        {
            let agents = self.agents.read().await;
            if agents.len() > (config.max_retained as usize + to_remove.len()) {
                let mut completed: Vec<_> = agents
                    .iter()
                    .filter(|(id, h)| {
                        h.status != SubAgentStatus::Running && !to_remove.contains(*id)
                    })
                    .collect();

                // Sort by last_activity (oldest first)
                completed.sort_by(|a, b| a.1.last_activity.cmp(&b.1.last_activity));

                let excess = agents.len() - (config.max_retained as usize + to_remove.len());
                for i in 0..excess.min(completed.len()) {
                    to_remove.push(completed[i].0.clone());
                }
            }
        }

        if to_remove.is_empty() {
            return;
        }

        tracing::info!("Cleaning up {} sub-agent(s)", to_remove.len());

        let mut agents = self.agents.write().await;
        for id in &to_remove {
            if let Some(handle) = agents.remove(id.as_str()) {
                self.cleanup_worktree(&handle);
            }
        }

        self.cleanup_completed(to_remove.len());
    }

    /// Remove a worktree directory for a completed sub-agent.
    fn cleanup_worktree(&self, handle: &SubAgentHandle) {
        if let Some(ref path) = handle.worktree_path {
            if std::path::Path::new(path).exists() {
                tracing::info!("Deleting sub-agent worktree at {:?}", path);
                // [d3vx Security Standard]: Careful with recursive deletes.
                // We check it's in /tmp/d3vx_worktrees first.
                if path.contains("d3vx_worktrees") {
                    // Use git worktree remove --force to ensure cleanup of both files and metadata
                    let status = std::process::Command::new("git")
                        .args(&["worktree", "remove", "--force", path])
                        .status();

                    if status.is_err() || !status.unwrap().success() {
                        // Fallback to manual deletion if git fails or is not in a repo
                        let _ = std::fs::remove_dir_all(path);
                    }
                }
            }
        }
    }

    fn cleanup_completed(&self, pruned_count: usize) {
        // Emit a global event to the broadcast channel
        let _ = self
            .broadcast_tx
            .send(super::super::agent_loop::AgentEvent::Cleanup { pruned_count });
        tracing::debug!(pruned = pruned_count, "Sub-agent cleanup cycle completed");
    }

    /// Start the background cleanup task
    pub fn start_cleanup_task(manager: Arc<super::SubAgentManager>, config: CleanupConfig) {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.cleanup_interval_secs));
            loop {
                interval.tick().await;
                manager.cleanup(&config).await;
            }
        });
    }
}
