//! Core reaction engine struct, constructors, and event processing.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::pipeline::heartbeat::HeartbeatManager;
use crate::pipeline::queue::TaskQueue;

use super::config::ReactionConfig;
use super::types::*;

// ============================================================================
// REACTION ENGINE
// ============================================================================

/// The main reaction engine
pub struct ReactionEngine {
    /// Configuration
    pub(super) config: ReactionConfig,
    /// Task queue for task operations
    pub(super) queue: Option<Arc<TaskQueue>>,
    /// Heartbeat manager for agent health
    pub(super) heartbeat_manager: Option<Arc<HeartbeatManager>>,
    /// Retry counts per task
    pub(super) retry_counts: RwLock<HashMap<String, u32>>,
    /// Audit trail
    pub(super) audit_trail: RwLock<Vec<ReactionAuditRecord>>,
    /// Statistics
    pub(super) stats: RwLock<ReactionStats>,
    /// Next audit record ID
    pub(super) next_audit_id: std::sync::atomic::AtomicU64,
}

impl ReactionEngine {
    /// Create a new reaction engine with default configuration
    pub fn new() -> Self {
        Self::with_config(ReactionConfig::default())
    }

    /// Create a new reaction engine with custom configuration
    pub fn with_config(config: ReactionConfig) -> Self {
        Self {
            config,
            queue: None,
            heartbeat_manager: None,
            retry_counts: RwLock::new(HashMap::new()),
            audit_trail: RwLock::new(Vec::new()),
            stats: RwLock::new(ReactionStats::default()),
            next_audit_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Set the task queue
    pub fn with_queue(mut self, queue: Arc<TaskQueue>) -> Self {
        self.queue = Some(queue);
        self
    }

    /// Set the heartbeat manager
    pub fn with_heartbeat_manager(mut self, manager: Arc<HeartbeatManager>) -> Self {
        self.heartbeat_manager = Some(manager);
        self
    }

    /// Process a reaction event and determine the appropriate action
    pub async fn process_event(&self, event: ReactionEvent) -> ReactionResult {
        // Check if reactions are globally enabled
        if !self.config.globally_enabled {
            return ReactionResult::new(
                event,
                ReactionType::NoAction,
                "Reactions are globally disabled".to_string(),
            );
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
        }

        // Determine reaction based on event type (handlers return decision without cloning event)
        let decision = match &event {
            ReactionEvent::CIFailure { .. } => self.handle_ci_failure(&event).await,
            ReactionEvent::ReviewComment { .. } => self.handle_review_comment(&event).await,
            ReactionEvent::MergeConflict { .. } => self.handle_merge_conflict(&event).await,
            ReactionEvent::AgentIdle { .. } => self.handle_agent_idle(&event).await,
            ReactionEvent::TaskFailed { .. } => self.handle_task_failed(&event).await,
        };

        // Build result once with owned event (single clone path for audit)
        let mut result = ReactionResult::new(event, decision.reaction, decision.reason);
        result.metadata = decision.metadata;

        // Record in audit trail
        self.record_audit(&result).await;

        result
    }

    /// Get configuration
    pub fn config(&self) -> &ReactionConfig {
        &self.config
    }

    /// Get retry count for a task
    pub(super) async fn get_retry_count(&self, event: &ReactionEvent) -> u32 {
        if let Some(task_id) = event.task_id() {
            let counts = self.retry_counts.read().await;
            counts.get(task_id).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Increment retry count for a task
    pub(super) async fn increment_retry_count(&self, event: &ReactionEvent) {
        if let Some(task_id) = event.task_id() {
            let mut counts = self.retry_counts.write().await;
            *counts.entry(task_id.to_string()).or_insert(0) += 1;
        }
    }

    /// Reset retry count for a task
    pub async fn reset_retry_count(&self, task_id: &str) {
        let mut counts = self.retry_counts.write().await;
        counts.remove(task_id);
    }
}

impl Default for ReactionEngine {
    fn default() -> Self {
        Self::new()
    }
}
