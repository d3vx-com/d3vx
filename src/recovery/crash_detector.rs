use crate::store::session::Session;
use std::time::Duration;

/// Status of a session's health
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashStatus {
    Healthy,
    Unresponsive,
    Crashed,
    Recovering,
}

/// Detects if a session has crashed or become unresponsive
pub struct CrashDetector {
    pub check_interval: Duration,
    pub max_idle_time: Duration,
}

impl Default for CrashDetector {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            max_idle_time: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl CrashDetector {
    pub fn new(check_interval: Duration, max_idle_time: Duration) -> Self {
        Self {
            check_interval,
            max_idle_time,
        }
    }

    /// Check the health of a session based on its last update time
    pub fn check_health(&self, session: &Session) -> CrashStatus {
        let now = chrono::Utc::now();
        let last_update = match chrono::DateTime::parse_from_rfc3339(&session.updated_at) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => return CrashStatus::Crashed, // Fallback
        };

        let idle_duration = now.signed_duration_since(last_update);

        if idle_duration.to_std().unwrap_or(Duration::ZERO) > self.max_idle_time {
            // If it's been idle too long without a state change to a terminal state
            match session.state {
                crate::store::session::SessionState::Stopped
                | crate::store::session::SessionState::Failed
                | crate::store::session::SessionState::Crashed
                | crate::store::session::SessionState::Cleaned
                | crate::store::session::SessionState::Abandoned => CrashStatus::Healthy,
                _ => CrashStatus::Unresponsive,
            }
        } else {
            CrashStatus::Healthy
        }
    }
}
