//! Memory configuration

use serde::{Deserialize, Serialize};

/// Persistent memory configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MemoryConfig {
    /// Enable persistent memory
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Directory for memory storage
    #[serde(default = "default_memory_dir")]
    pub dir: String,
    /// Maximum entries in memory index
    #[serde(default = "default_max_entries")]
    pub max_entries: u32,
    /// Automatically extract knowledge after tasks
    #[serde(default = "default_true")]
    pub auto_learn: bool,
    /// Enable FTS5 full-text search
    #[serde(default = "default_true")]
    pub enable_search: bool,
}

fn default_true() -> bool {
    true
}

fn default_memory_dir() -> String {
    ".d3vx/memory".to_string()
}

fn default_max_entries() -> u32 {
    10000
}
