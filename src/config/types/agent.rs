//! Agent enhancement configuration (compaction, doom loop, best-of-n, skills)

use serde::{Deserialize, Serialize};

/// Configuration for agent enhancements
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AgentEnhancementsConfig {
    /// Context compaction settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionSettings>,
    /// Doom loop detection settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doom_loop: Option<DoomLoopSettings>,
    /// Best-of-N settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_of_n: Option<BestOfNSettings>,
    /// Skills settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsSettings>,
}

/// Compaction settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CompactionSettings {
    /// Enable automatic compaction
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Token threshold ratio to trigger compaction (0.0 - 1.0)
    #[serde(default = "default_compaction_threshold")]
    pub threshold_ratio: f64,
    /// Number of recent messages to keep during compaction
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,
    /// Minimum messages to compact at once
    #[serde(default = "default_min_compact")]
    pub min_compact_count: usize,
}

/// Doom loop detection settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DoomLoopSettings {
    /// Enable doom loop detection
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Number of times a pattern must repeat before warning
    #[serde(default = "default_doom_threshold")]
    pub threshold: usize,
    /// Number of recent tool calls to track
    #[serde(default = "default_doom_window")]
    pub window_size: usize,
    /// Minimum interval between warnings (seconds)
    #[serde(default = "default_warning_interval")]
    pub warning_interval_secs: u64,
}

/// Best-of-N settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BestOfNSettings {
    /// Default number of variants
    #[serde(default = "default_n_variants")]
    pub n: usize,
    /// Whether to strip reasoning from output
    #[serde(default = "default_strip_reasoning")]
    pub strip_reasoning: bool,
    /// Custom selector prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector_prompt: Option<String>,
}

/// Skills settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SkillsSettings {
    /// Enable skills system
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Search paths for skills
    #[serde(default)]
    pub search_paths: Vec<String>,
    /// Auto-load skills on startup
    #[serde(default = "default_auto_load_skills")]
    pub auto_load: bool,
}

fn default_true() -> bool {
    true
}

fn default_compaction_threshold() -> f64 {
    0.80
}

fn default_keep_recent() -> usize {
    10
}

fn default_min_compact() -> usize {
    3
}

fn default_doom_threshold() -> usize {
    3
}

fn default_doom_window() -> usize {
    10
}

fn default_warning_interval() -> u64 {
    30
}

fn default_n_variants() -> usize {
    3
}

fn default_strip_reasoning() -> bool {
    true
}

fn default_auto_load_skills() -> bool {
    false
}
