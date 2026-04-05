//! UI configuration

use serde::{Deserialize, Serialize};

/// UI configuration for multi-view support
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct UiConfig {
    /// Default UI mode
    #[serde(default)]
    pub mode: super::app::UiMode,
    /// Enable autonomous task execution
    #[serde(default)]
    pub autonomous: bool,
    /// Show floating status bar
    #[serde(default = "default_true")]
    pub floating_status: bool,
    /// Auto-switch to kanban when tasks exist
    #[serde(default)]
    pub auto_switch_on_tasks: bool,
    /// Refresh interval for UI updates (ms)
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_ms: u64,
    /// Show help footer in views
    #[serde(default = "default_true")]
    pub show_help_footer: bool,
    /// Default Power Mode (telemetry HUD)
    #[serde(default)]
    pub power_mode: bool,
    /// Show welcome banner on startup
    #[serde(default = "default_true")]
    pub show_welcome: bool,
    /// Default sidebar width
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
}

fn default_true() -> bool {
    true
}

fn default_refresh_interval() -> u64 {
    1000
}

fn default_sidebar_width() -> u16 {
    30
}
