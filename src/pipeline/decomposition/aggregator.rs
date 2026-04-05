//! Result aggregator for combining child results

use super::types::{AggregationStrategy, ChildTaskStatus, DecompositionStatus};
use crate::pipeline::phases::TaskStatus;

/// Result aggregator for combining child results
pub struct ResultAggregator {
    /// Aggregation strategy
    strategy: AggregationStrategy,
}

impl ResultAggregator {
    /// Create a new aggregator
    pub fn new(strategy: AggregationStrategy) -> Self {
        Self { strategy }
    }

    /// Aggregate child results into a parent result
    pub fn aggregate(&self, child_statuses: &[ChildTaskStatus]) -> (DecompositionStatus, String) {
        let total = child_statuses.len();
        if total == 0 {
            return (
                DecompositionStatus::Failed,
                "No child tasks to aggregate".to_string(),
            );
        }

        let succeeded = child_statuses
            .iter()
            .filter(|c| c.status == TaskStatus::Completed)
            .count();

        let failed = child_statuses
            .iter()
            .filter(|c| c.status == TaskStatus::Failed)
            .count();

        let result = match self.strategy {
            AggregationStrategy::AllSuccess => {
                if succeeded == total {
                    (
                        DecompositionStatus::Completed,
                        format!("All {} child tasks completed successfully", total),
                    )
                } else {
                    (
                        DecompositionStatus::Failed,
                        format!("{} of {} child tasks failed", failed, total),
                    )
                }
            }
            AggregationStrategy::MajoritySuccess => {
                if succeeded > total / 2 {
                    (
                        DecompositionStatus::Completed,
                        format!(
                            "{} of {} child tasks succeeded (majority)",
                            succeeded, total
                        ),
                    )
                } else {
                    (
                        DecompositionStatus::Failed,
                        format!(
                            "Only {} of {} child tasks succeeded (need majority)",
                            succeeded, total
                        ),
                    )
                }
            }
            AggregationStrategy::AnySuccess => {
                if succeeded > 0 {
                    (
                        DecompositionStatus::Completed,
                        format!("{} of {} child tasks succeeded", succeeded, total),
                    )
                } else {
                    (
                        DecompositionStatus::Failed,
                        "All child tasks failed".to_string(),
                    )
                }
            }
            AggregationStrategy::Custom => {
                // Custom logic would be provided per decomposition
                if succeeded == total {
                    (
                        DecompositionStatus::Completed,
                        format!("All {} child tasks completed", total),
                    )
                } else if succeeded > 0 {
                    (
                        DecompositionStatus::Partial,
                        format!("{} succeeded, {} failed", succeeded, failed),
                    )
                } else {
                    (
                        DecompositionStatus::Failed,
                        "All child tasks failed".to_string(),
                    )
                }
            }
        };

        result
    }
}
