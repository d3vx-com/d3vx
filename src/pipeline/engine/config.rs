//! Pipeline engine types and configuration
//!
//! Defines callbacks and configuration types for the pipeline engine.

use std::sync::Arc;

use super::super::handlers::PhaseResult;
use super::super::phases::{Task, TaskStatus};

/// Callback for phase completion events
pub type PhaseCallback =
    Arc<dyn Fn(&Task, super::super::phases::Phase, &PhaseResult) + Send + Sync>;

/// Callback for status change events
pub type StatusCallback = Arc<dyn Fn(&Task, TaskStatus) + Send + Sync>;

/// Configuration for the pipeline engine
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Whether to auto-commit after each phase
    pub auto_commit: bool,
    /// Maximum cost per task in USD
    pub max_cost_usd: Option<f64>,
    /// Timeout for pipeline execution in minutes
    pub timeout_minutes: u64,
    /// Whether to enable checkpoint recovery
    pub enable_checkpoints: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            auto_commit: true,
            max_cost_usd: None,
            timeout_minutes: 60,
            enable_checkpoints: true,
        }
    }
}
