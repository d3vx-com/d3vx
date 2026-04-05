//! Notification types for human intervention alerts.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::types::{ReactionResult, ReactionType};

// ============================================================================
// NOTIFICATION TYPES
// ============================================================================

/// Notification payload for human intervention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    /// Notification title
    pub title: String,
    /// Notification message
    pub message: String,
    /// Severity level
    pub severity: NotificationSeverity,
    /// Associated task ID
    pub task_id: Option<String>,
    /// Event that triggered the notification
    pub event_type: String,
    /// Recommended action
    pub recommended_action: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Notification severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for NotificationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationSeverity::Info => write!(f, "info"),
            NotificationSeverity::Warning => write!(f, "warning"),
            NotificationSeverity::Error => write!(f, "error"),
            NotificationSeverity::Critical => write!(f, "critical"),
        }
    }
}

impl NotificationPayload {
    /// Create a new notification payload from a reaction result
    pub fn from_result(result: &ReactionResult) -> Self {
        let severity = match result.reaction {
            ReactionType::Notify => NotificationSeverity::Warning,
            ReactionType::Escalate => NotificationSeverity::Critical,
            ReactionType::Cancel => NotificationSeverity::Error,
            _ => NotificationSeverity::Info,
        };

        let recommended_action = match result.reaction {
            ReactionType::Notify => Some("Review the issue and decide on next steps".to_string()),
            ReactionType::Escalate => Some("Immediate attention required".to_string()),
            ReactionType::Cancel => {
                Some("Task was cancelled, consider manual intervention".to_string())
            }
            _ => None,
        };

        Self {
            title: format!("Reaction: {}", result.event.event_type()),
            message: result.reason.clone(),
            severity,
            task_id: result.event.task_id().map(String::from),
            event_type: result.event.event_type().to_string(),
            recommended_action,
            timestamp: chrono::Utc::now(),
            context: result.metadata.clone(),
        }
    }
}
