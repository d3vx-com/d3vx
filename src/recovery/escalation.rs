use std::time::Duration;

/// Levels of escalation for error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationLevel {
    None,
    Retry,   // Automatic retry
    Backoff, // Exponential backoff
    Restore, // Restore from checkpoint
    Human,   // Escalate to human
    Abort,   // Give up
}

/// Strategy for escalating recovery attempts
#[derive(Debug, Clone)]
pub struct EscalationStrategy {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub checkpoint_enabled: bool,
}

impl Default for EscalationStrategy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            checkpoint_enabled: true,
        }
    }
}

impl EscalationStrategy {
    /// Calculate the next action based on current failure count
    pub fn next_action(&self, failure_count: u32) -> EscalationLevel {
        if failure_count == 0 {
            EscalationLevel::None
        } else if failure_count <= self.max_retries {
            EscalationLevel::Retry
        } else if failure_count == self.max_retries + 1 {
            EscalationLevel::Backoff
        } else if failure_count == self.max_retries + 2 && self.checkpoint_enabled {
            EscalationLevel::Restore
        } else if failure_count <= self.max_retries + 4 {
            EscalationLevel::Human
        } else {
            EscalationLevel::Abort
        }
    }

    /// Calculate the delay for a specific failure count using exponential backoff
    pub fn get_delay(&self, failure_count: u32) -> Duration {
        if failure_count == 0 {
            return Duration::ZERO;
        }

        let delay_ms = (self.initial_delay.as_millis() as f64)
            * self.backoff_multiplier.powi(failure_count as i32 - 1);
        let duration = Duration::from_millis(delay_ms as u64);

        if duration > self.max_delay {
            self.max_delay
        } else {
            duration
        }
    }
}
