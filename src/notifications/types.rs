//! Notification types shared across dispatchers.

use serde::{Deserialize, Serialize};

/// Notification priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Info,
    Warning,
    Action,
    Urgent,
}

/// Channel configuration for dispatching notifications.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Telegram {
        bot_token_env: String,
        chat_id_env: String,
    },
    Webhook {
        url: String,
        secret: Option<String>,
    },
}

/// A notification ready to be dispatched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub metadata: serde_json::Value,
}

impl Notification {
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            priority: NotificationPriority::Info,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn urgent(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            priority: NotificationPriority::Urgent,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        if self.metadata.is_null() {
            self.metadata = serde_json::json!({});
        }
        if let Some(map) = self.metadata.as_object_mut() {
            map.insert(key.to_string(), value);
        }
        self
    }
}

/// Result of a single dispatch attempt.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub channel: String,
    pub success: bool,
    pub error: Option<String>,
}
