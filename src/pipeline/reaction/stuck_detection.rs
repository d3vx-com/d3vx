//! Stuck agent detection via heartbeat monitoring.

use super::engine::ReactionEngine;
use super::types::*;

impl ReactionEngine {
    /// Check for stuck agents using heartbeat manager
    pub async fn check_stuck_agents(&self) -> Vec<ReactionEvent> {
        let Some(heartbeat_manager) = &self.heartbeat_manager else {
            return Vec::new();
        };

        let config = &self.config.agent_idle;
        if !config.enabled {
            return Vec::new();
        }

        let stale_workers: Vec<_> = heartbeat_manager.detect_stale_workers().await;
        stale_workers
            .into_iter()
            .filter_map(|worker| {
                worker.task_id.map(|task_id| ReactionEvent::AgentIdle {
                    worker_id: worker.worker_id.0,
                    task_id,
                    idle_duration_secs: worker.last_heartbeat_ago.as_secs(),
                    last_phase: None,
                })
            })
            .collect()
    }

    /// Process all stuck agents
    pub async fn process_stuck_agents(&self) -> Vec<ReactionResult> {
        let events = self.check_stuck_agents().await;
        let mut results = Vec::new();
        for event in events {
            let result = self.process_event(event).await;
            results.push(result);
        }
        results
    }
}
