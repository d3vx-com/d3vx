//! Budget guardrails configuration

use serde::{Deserialize, Serialize};

/// Budget guardrails configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BudgetConfig {
    /// Per-session budget limit in USD (0 = disabled)
    #[serde(default = "default_per_session")]
    pub per_session: f64,
    /// Per-day budget limit in USD (0 = disabled)
    #[serde(default = "default_per_day")]
    pub per_day: f64,
    /// Warn when spend reaches this fraction of limit
    #[serde(default = "default_warn_at")]
    pub warn_at: f64,
    /// Pause execution when limit reached
    #[serde(default = "default_pause_at")]
    pub pause_at: f64,
    /// Enable budget enforcement
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_per_session() -> f64 {
    5.00
}

fn default_per_day() -> f64 {
    50.00
}

fn default_warn_at() -> f64 {
    0.8
}

fn default_pause_at() -> f64 {
    1.0
}
