//! Sub-agent configuration

use serde::{Deserialize, Serialize};

/// Configuration for sub-agent resource management and cleanup
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CleanupConfig {
    /// How long to keep completed sub-agent handles before pruning (seconds)
    #[serde(default = "default_retention_period")]
    pub retention_period_secs: u64,
    /// Interval for the background cleanup task (seconds)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,
    /// Maximum number of completed handles to retain
    #[serde(default = "default_max_retained")]
    pub max_retained: u32,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            retention_period_secs: 300,
            cleanup_interval_secs: 60,
            max_retained: 10,
        }
    }
}

/// Configuration for sub-agent behavior
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct SubAgentConfig {
    /// Resource cleanup settings
    #[serde(default)]
    pub cleanup: CleanupConfig,
    /// Enable parallel agent execution for complex tasks
    #[serde(default)]
    pub parallel_agents: bool,
    /// Maximum number of parallel agents to spawn
    #[serde(default = "default_max_parallel_agents")]
    pub max_parallel_agents: u32,
    /// Enable auto-detection of task complexity for parallel execution
    #[serde(default = "default_true")]
    pub auto_detect_complexity: bool,
}

fn default_true() -> bool {
    true
}

fn default_retention_period() -> u64 {
    300
}

fn default_cleanup_interval() -> u64 {
    60
}

fn default_max_retained() -> u32 {
    10
}

fn default_max_parallel_agents() -> u32 {
    3
}
