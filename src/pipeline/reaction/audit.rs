//! Audit trail recording and statistics.

use super::engine::ReactionEngine;
use super::types::*;

impl ReactionEngine {
    /// Record an audit entry
    pub(super) async fn record_audit(&self, result: &ReactionResult) {
        let id = self
            .next_audit_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let record = ReactionAuditRecord {
            id: format!("REACTION-{}", id),
            timestamp: chrono::Utc::now(),
            event: result.event.clone(),
            reaction: result.reaction,
            reason: result.reason.clone(),
            success: result.executed,
            task_id: result.event.task_id().map(String::from),
            retry_count: self.get_retry_count(&result.event).await,
        };

        let mut trail = self.audit_trail.write().await;
        trail.push(record);

        // Keep only last 1000 records
        if trail.len() > 1000 {
            let excess = trail.len() - 1000;
            trail.drain(0..excess);
        }
    }

    /// Get audit trail
    pub async fn get_audit_trail(&self) -> Vec<ReactionAuditRecord> {
        self.audit_trail.read().await.clone()
    }

    /// Get audit trail for a specific task
    pub async fn get_audit_trail_for_task(&self, task_id: &str) -> Vec<ReactionAuditRecord> {
        self.audit_trail
            .read()
            .await
            .iter()
            .filter(|r| r.task_id.as_deref() == Some(task_id))
            .cloned()
            .collect()
    }

    /// Get statistics
    pub async fn stats(&self) -> ReactionStats {
        self.stats.read().await.clone()
    }

    /// Update statistics based on reaction result
    pub async fn record_result(&self, result: &ReactionResult) {
        let mut stats = self.stats.write().await;
        match result.reaction {
            ReactionType::AutoFix => {
                if result.executed {
                    stats.auto_fix_successes += 1;
                }
            }
            ReactionType::Notify => {
                stats.notifications_sent += 1;
            }
            ReactionType::Escalate => {
                stats.escalations += 1;
            }
            ReactionType::Checkpoint => {
                stats.checkpoints_created += 1;
            }
            ReactionType::Cancel => {
                stats.tasks_cancelled += 1;
            }
            ReactionType::NoAction => {}
        }
    }
}
