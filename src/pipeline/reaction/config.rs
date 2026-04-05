//! Configuration types for the reaction engine.

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ============================================================================
// CI FAILURE CONFIG
// ============================================================================

/// Configuration for CI failure reactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIFailureConfig {
    /// Whether CI failure reactions are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum auto-fix retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Whether to attempt automatic fixes
    #[serde(default = "default_true")]
    pub auto_fix: bool,
    /// Whether to notify human after max retries exceeded
    #[serde(default = "default_true")]
    pub notify_on_failure: bool,
    /// Cooldown period between retries (seconds)
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
}

impl Default for CIFailureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            auto_fix: true,
            notify_on_failure: true,
            cooldown_secs: 60,
        }
    }
}

// ============================================================================
// REVIEW COMMENT CONFIG
// ============================================================================

/// Configuration for review comment reactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCommentConfig {
    /// Whether review comment reactions are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether to auto-fix trivial issues
    #[serde(default = "default_true")]
    pub auto_fix_trivial: bool,
    /// Whether to notify human for complex issues
    #[serde(default = "default_true")]
    pub notify_on_complex: bool,
    /// Keywords that indicate trivial issues
    #[serde(default = "default_trivial_keywords")]
    pub trivial_keywords: Vec<String>,
    /// Keywords that indicate complex issues
    #[serde(default = "default_complex_keywords")]
    pub complex_keywords: Vec<String>,
}

impl Default for ReviewCommentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_fix_trivial: true,
            notify_on_complex: true,
            trivial_keywords: default_trivial_keywords(),
            complex_keywords: default_complex_keywords(),
        }
    }
}

// ============================================================================
// MERGE CONFLICT CONFIG
// ============================================================================

/// Configuration for merge conflict reactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflictConfig {
    /// Whether merge conflict reactions are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether to attempt automatic resolution
    #[serde(default)]
    pub auto_resolve: bool,
    /// Whether to always notify human
    #[serde(default = "default_true")]
    pub notify_always: bool,
    /// Maximum auto-resolution attempts
    #[serde(default = "default_max_retries")]
    pub max_resolution_attempts: u32,
}

impl Default for MergeConflictConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_resolve: false,
            notify_always: true,
            max_resolution_attempts: 1,
        }
    }
}

// ============================================================================
// AGENT IDLE CONFIG
// ============================================================================

/// Configuration for agent idle detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdleConfig {
    /// Whether agent idle reactions are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Seconds of idle time before considering agent stuck
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Whether to notify when agent is stuck
    #[serde(default = "default_true")]
    pub notify_on_stuck: bool,
    /// Whether to checkpoint before notifying
    #[serde(default = "default_true")]
    pub checkpoint_before_notify: bool,
    /// Maximum time before forcing cancellation (seconds)
    #[serde(default = "default_max_idle")]
    pub max_idle_secs: u64,
}

impl Default for AgentIdleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_timeout_secs: 300,
            notify_on_stuck: true,
            checkpoint_before_notify: true,
            max_idle_secs: 3600,
        }
    }
}

// ============================================================================
// FULL REACTION CONFIG
// ============================================================================

/// Full reaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionConfig {
    /// CI failure reaction config
    #[serde(default)]
    pub ci_failure: CIFailureConfig,
    /// Review comment reaction config
    #[serde(default)]
    pub review_comment: ReviewCommentConfig,
    /// Merge conflict reaction config
    #[serde(default)]
    pub merge_conflict: MergeConflictConfig,
    /// Agent idle reaction config
    #[serde(default)]
    pub agent_idle: AgentIdleConfig,
    /// Whether all reactions are globally enabled
    #[serde(default = "default_true")]
    pub globally_enabled: bool,
    /// Default notification channel (e.g., "slack", "email")
    #[serde(default)]
    pub notification_channel: Option<String>,
}

impl Default for ReactionConfig {
    fn default() -> Self {
        Self {
            ci_failure: CIFailureConfig::default(),
            review_comment: ReviewCommentConfig::default(),
            merge_conflict: MergeConflictConfig::default(),
            agent_idle: AgentIdleConfig::default(),
            globally_enabled: true,
            notification_channel: None,
        }
    }
}

impl ReactionConfig {
    /// Create a new config with CI failure settings
    pub fn with_ci_failure(mut self, config: CIFailureConfig) -> Self {
        self.ci_failure = config;
        self
    }

    /// Create a new config with review comment settings
    pub fn with_review_comment(mut self, config: ReviewCommentConfig) -> Self {
        self.review_comment = config;
        self
    }

    /// Create a new config with merge conflict settings
    pub fn with_merge_conflict(mut self, config: MergeConflictConfig) -> Self {
        self.merge_conflict = config;
        self
    }

    /// Create a new config with agent idle settings
    pub fn with_agent_idle(mut self, config: AgentIdleConfig) -> Self {
        self.agent_idle = config;
        self
    }

    /// Disable all reactions
    pub fn disabled() -> Self {
        Self {
            globally_enabled: false,
            ..Default::default()
        }
    }

    /// Load from YAML file
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(yaml)?;
        Ok(config)
    }

    /// Convert to YAML
    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }
}

// ============================================================================
// DEFAULT HELPERS
// ============================================================================

fn default_true() -> bool {
    true
}
fn default_max_retries() -> u32 {
    3
}
fn default_cooldown() -> u64 {
    60
}
fn default_idle_timeout() -> u64 {
    300
}
fn default_max_idle() -> u64 {
    3600
}
fn default_trivial_keywords() -> Vec<String> {
    vec![
        "typo".to_string(),
        "nit".to_string(),
        "formatting".to_string(),
        "whitespace".to_string(),
        "rename".to_string(),
        "minor".to_string(),
    ]
}
fn default_complex_keywords() -> Vec<String> {
    vec![
        "architectural".to_string(),
        "security".to_string(),
        "breaking change".to_string(),
        "critical".to_string(),
        "major refactor".to_string(),
        "design decision".to_string(),
    ]
}
