//! Metrics and Cost Tracking Module
//!
//! Handles API usage recording, cost estimation, and session statistics.

use anyhow::Result;

use std::sync::Arc;

use super::cost_tracker::{estimate_cost, ApiUsage, CostStats, CostTracker, CostTrackerConfig};
use super::engine::PipelineRunResult;

pub struct MetricsCollector {
    cost_tracker: Arc<CostTracker>,
}

impl MetricsCollector {
    pub fn new(config: CostTrackerConfig) -> Self {
        Self {
            cost_tracker: Arc::new(CostTracker::with_config(config)),
        }
    }

    pub fn cost_tracker(&self) -> Arc<CostTracker> {
        self.cost_tracker.clone()
    }

    pub async fn get_stats(&self) -> CostStats {
        self.cost_tracker.get_session_stats().await
    }

    pub async fn record_run_result(&self, result: &PipelineRunResult) -> Result<()> {
        for (phase, phase_result) in &result.phase_results {
            if let Some(metadata) = phase_result.metadata.as_object() {
                let input: Option<u64> = metadata.get("input_tokens").and_then(|v| v.as_u64());
                let output: Option<u64> = metadata.get("output_tokens").and_then(|v| v.as_u64());
                let model: Option<&str> = metadata.get("model").and_then(|v| v.as_str());

                if let (Some(input), Some(output)) = (input, output) {
                    let model = model.unwrap_or("unknown").to_string();

                    let cost = estimate_cost(&model, input, output);
                    let usage = ApiUsage {
                        input_tokens: input,
                        output_tokens: output,
                        cost_usd: cost,
                        model,
                        phase: *phase,
                        timestamp: chrono::Utc::now(),
                    };

                    self.cost_tracker
                        .record_usage(&result.task.id, usage)
                        .await?;
                }
            }
        }
        Ok(())
    }
}
