//! Recovery and checkpoint configuration

use serde::{Deserialize, Serialize};

/// Configuration for error recovery and session restoration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct RecoveryConfig {
    /// Maximum retries for automatic recovery
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial delay for exponential backoff (ms)
    #[serde(default = "default_initial_delay")]
    pub initial_delay_ms: u32,
    /// Maximum delay for exponential backoff (ms)
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u32,
    /// Multiplier for exponential backoff
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Whether to enable checkpoint restoration
    #[serde(default = "default_true")]
    pub checkpoint_enabled: bool,
    /// Timeout for human escalation in milliseconds
    #[serde(default = "default_human_timeout")]
    pub human_escalation_timeout: u64,
    /// Check interval for crash detection (ms)
    #[serde(default = "default_check_interval")]
    pub check_interval_ms: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 500,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            checkpoint_enabled: true,
            human_escalation_timeout: 300000,
            check_interval_ms: 30000,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay() -> u32 {
    500
}

fn default_max_delay() -> u32 {
    30000
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_human_timeout() -> u64 {
    300000
}

fn default_check_interval() -> u64 {
    30000
}
