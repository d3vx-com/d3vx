//! Cost tracker implementation
//!
//! Tracks API usage costs and enforces budget limits.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::types::*;

/// Tracks API usage costs
pub struct CostTracker {
    /// Configuration
    pub(super) config: CostTrackerConfig,
    /// Usage records by task ID
    usage_by_task: Arc<RwLock<HashMap<String, Vec<ApiUsage>>>>,
    /// Session-wide usage
    session_usage: Arc<RwLock<Vec<ApiUsage>>>,
    /// Total session stats (cached)
    session_stats: Arc<RwLock<CostStats>>,
}

impl CostTracker {
    /// Create a new cost tracker with default configuration
    pub fn new() -> Self {
        Self::with_config(CostTrackerConfig::default())
    }

    /// Create a new cost tracker with custom configuration
    pub fn with_config(config: CostTrackerConfig) -> Self {
        Self {
            config,
            usage_by_task: Arc::new(RwLock::new(HashMap::new())),
            session_usage: Arc::new(RwLock::new(Vec::new())),
            session_stats: Arc::new(RwLock::new(CostStats::default())),
        }
    }

    /// Record API usage for a task
    pub async fn record_usage(
        &self,
        task_id: &str,
        usage: ApiUsage,
    ) -> Result<(), CostTrackerError> {
        // Check budget limits before recording
        self.check_budget(&usage).await?;

        // Add to task-specific records
        let mut task_usage = self.usage_by_task.write().await;
        task_usage
            .entry(task_id.to_string())
            .or_insert_with(Vec::new)
            .push(usage.clone());

        // Add to session-wide records
        let mut session_usage = self.session_usage.write().await;
        session_usage.push(usage.clone());

        // Update session stats
        self.update_session_stats(&usage).await;

        debug!(
            "Recorded usage for task {}: {} input + {} output tokens = ${:.4}",
            task_id, usage.input_tokens, usage.output_tokens, usage.cost_usd
        );

        Ok(())
    }

    /// Get cost statistics for a specific task
    pub async fn get_task_stats(&self, task_id: &str) -> CostStats {
        let task_usage = self.usage_by_task.read().await;
        let usage = match task_usage.get(task_id) {
            Some(u) => u,
            None => return CostStats::default(),
        };

        self.calculate_stats(usage)
    }

    /// Get session-wide cost statistics
    pub async fn get_session_stats(&self) -> CostStats {
        let stats = self.session_stats.read().await;
        stats.clone()
    }

    /// Check if a task is within budget
    pub async fn is_within_task_budget(&self, task_id: &str) -> bool {
        if let Some(max_cost) = self.config.max_task_cost {
            let stats = self.get_task_stats(task_id).await;
            stats.total_cost_usd < max_cost
        } else {
            true
        }
    }

    /// Check if the session is within budget
    pub async fn is_within_session_budget(&self) -> bool {
        if let Some(max_cost) = self.config.max_session_cost {
            let stats = self.get_session_stats().await;
            stats.total_cost_usd < max_cost
        } else {
            true
        }
    }

    /// Get remaining budget for a task
    pub async fn get_remaining_task_budget(&self, task_id: &str) -> Option<f64> {
        if let Some(max) = self.config.max_task_cost {
            let stats = self.get_task_stats(task_id).await;
            Some((max - stats.total_cost_usd).max(0.0))
        } else {
            None
        }
    }

    /// Get remaining budget for the session
    pub async fn get_remaining_session_budget(&self) -> Option<f64> {
        if let Some(max) = self.config.max_session_cost {
            let stats = self.get_session_stats().await;
            Some((max - stats.total_cost_usd).max(0.0))
        } else {
            None
        }
    }

    /// Clear all tracking data
    pub async fn clear(&self) {
        let mut task_usage = self.usage_by_task.write().await;
        task_usage.clear();

        let mut session_usage = self.session_usage.write().await;
        session_usage.clear();

        let mut session_stats = self.session_stats.write().await;
        *session_stats = CostStats::default();

        info!("Cleared all cost tracking data");
    }

    /// Export usage data as JSON
    pub async fn export_json(&self) -> Result<String> {
        let session_usage = self.session_usage.read().await;
        let json = serde_json::to_string_pretty(&*session_usage)?;
        Ok(json)
    }

    /// Calculate statistics from usage records
    fn calculate_stats(&self, usage: &[ApiUsage]) -> CostStats {
        let mut stats = CostStats::default();

        for u in usage {
            stats.total_input_tokens += u.input_tokens;
            stats.total_output_tokens += u.output_tokens;
            stats.total_cost_usd += u.cost_usd;
            stats.api_calls += 1;

            if self.config.track_by_phase {
                *stats
                    .cost_by_phase
                    .entry(u.phase.to_string())
                    .or_insert(0.0) += u.cost_usd;
            }

            if self.config.track_by_model {
                let entry = stats
                    .tokens_by_model
                    .entry(u.model.clone())
                    .or_insert((0, 0));
                entry.0 += u.input_tokens;
                entry.1 += u.output_tokens;
            }
        }

        stats
    }

    /// Update session-wide statistics
    async fn update_session_stats(&self, usage: &ApiUsage) {
        let mut stats = self.session_stats.write().await;

        stats.total_input_tokens += usage.input_tokens;
        stats.total_output_tokens += usage.output_tokens;
        stats.total_cost_usd += usage.cost_usd;
        stats.api_calls += 1;

        if self.config.track_by_phase {
            *stats
                .cost_by_phase
                .entry(usage.phase.to_string())
                .or_insert(0.0) += usage.cost_usd;
        }

        if self.config.track_by_model {
            let entry = stats
                .tokens_by_model
                .entry(usage.model.clone())
                .or_insert((0, 0));
            entry.0 += usage.input_tokens;
            entry.1 += usage.output_tokens;
        }
    }

    /// Check if recording usage would exceed budget
    async fn check_budget(&self, usage: &ApiUsage) -> Result<(), CostTrackerError> {
        // Check session budget
        if let Some(max_session_cost) = self.config.max_session_cost {
            let stats = self.session_stats.read().await;
            let new_total = stats.total_cost_usd + usage.cost_usd;

            if new_total > max_session_cost {
                error!(
                    "Session budget exceeded: ${:.2} > ${:.2}",
                    new_total, max_session_cost
                );
                return Err(CostTrackerError::BudgetExceeded {
                    actual: new_total,
                    limit: max_session_cost,
                });
            }
        }

        Ok(())
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}
