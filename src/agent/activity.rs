//! Session Activity Tracker
//!
//! Tracks agent activity for monitoring and metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub timestamp: DateTime<Utc>,
    pub activity_type: ActivityType,
    pub description: String,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Thinking,
    ToolExecution,
    WaitingForPermission,
    Idle,
    Completed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    Active,
    Idle,
    WaitingInput,
    Blocked,
    Exited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySummary {
    pub session_id: String,
    pub state: ActivityState,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub total_events: usize,
    pub tool_summary: Vec<ToolActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolActivity {
    pub tool_name: String,
    pub count: usize,
    pub total_duration_ms: u64,
}

pub struct ActivityTracker {
    session_id: String,
    state: RwLock<ActivityState>,
    started_at: DateTime<Utc>,
    last_activity: RwLock<DateTime<Utc>>,
    events: RwLock<VecDeque<ActivityEvent>>,
    tool_counts: RwLock<std::collections::HashMap<String, (usize, u64)>>,
}

impl ActivityTracker {
    pub fn new(session_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            state: RwLock::new(ActivityState::Active),
            started_at: now,
            last_activity: RwLock::new(now),
            events: RwLock::new(VecDeque::with_capacity(1000)),
            tool_counts: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub async fn record(&self, activity_type: ActivityType, description: impl Into<String>) {
        let now = Utc::now();
        let event = ActivityEvent {
            timestamp: now,
            activity_type,
            description: description.into(),
            duration_ms: None,
        };

        let mut events = self.events.write().await;
        if events.len() >= 1000 {
            events.pop_front();
        }
        events.push_back(event);

        *self.last_activity.write().await = now;
        *self.state.write().await = ActivityState::Active;
    }

    pub async fn record_tool(&self, tool_name: &str, duration_ms: u64) {
        let mut counts = self.tool_counts.write().await;
        let entry = counts.entry(tool_name.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += duration_ms;
        self.record(ActivityType::ToolExecution, tool_name).await;
    }

    pub async fn set_state(&self, state: ActivityState) {
        *self.state.write().await = state;
        *self.last_activity.write().await = Utc::now();
    }

    pub async fn summary(&self) -> ActivitySummary {
        let events = self.events.read().await;
        let tool_counts = self.tool_counts.read().await;
        let last = *self.last_activity.read().await;

        let tool_summary: Vec<ToolActivity> = tool_counts.iter()
            .map(|(name, (count, duration))| ToolActivity {
                tool_name: name.clone(),
                count: *count,
                total_duration_ms: *duration,
            })
            .collect();

        ActivitySummary {
            session_id: self.session_id.clone(),
            state: *self.state.read().await,
            started_at: self.started_at,
            last_activity: last,
            total_events: events.len(),
            tool_summary,
        }
    }
}
