//! Pipeline configuration

use serde::{Deserialize, Serialize};

/// Configuration for a single pipeline phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PipelinePhase {
    /// Whether this phase is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum retries for this phase
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Specific model to use for this phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Timeout for this phase in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Configuration for all pipeline phases
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PipelinePhases {
    pub research: PipelinePhase,
    pub plan: PipelinePhase,
    pub implement: PipelinePhase,
    pub review: PipelinePhase,
    pub docs: PipelinePhase,
    pub learn: PipelinePhase,
}

/// Budget limits for the pipeline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PipelineBudget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_per_task_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_per_batch_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_at_usd: Option<f64>,
}

/// Watchdog timeouts for the pipeline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PipelineTimeouts {
    #[serde(default = "default_pipeline_minutes")]
    pub pipeline_minutes: u64,
    #[serde(default = "default_phase_minutes")]
    pub phase_minutes: u64,
}

/// Top-level pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PipelineConfig {
    pub phases: PipelinePhases,
    /// Stop pipeline on first failure
    #[serde(default = "default_true")]
    pub stop_on_failure: bool,
    /// Generate checkpoint after each phase
    #[serde(default = "default_true")]
    pub checkpoint: bool,
    /// Budget limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<PipelineBudget>,
    /// Watchdog timeouts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeouts: Option<PipelineTimeouts>,
    /// Max concurrent agents
    #[serde(default = "default_max_concurrent_agents")]
    pub max_concurrent_agents: u32,
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u32 {
    2
}

fn default_pipeline_minutes() -> u64 {
    60
}

fn default_phase_minutes() -> u64 {
    20
}

fn default_max_concurrent_agents() -> u32 {
    3
}
