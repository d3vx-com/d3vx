//! Notifications configuration

use serde::{Deserialize, Serialize};

/// Telegram configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

/// Notifications configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct NotificationsConfig {
    /// Enable desktop notifications
    #[serde(default = "default_true")]
    pub desktop: bool,
    /// Telegram configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramConfig>,
    /// Notify on task done
    #[serde(default = "default_true")]
    pub on_task_done: bool,
    /// Notify on task failed
    #[serde(default = "default_true")]
    pub on_task_failed: bool,
    /// Notify when mergeable
    #[serde(default = "default_true")]
    pub on_mergeable: bool,
}

fn default_true() -> bool {
    true
}
