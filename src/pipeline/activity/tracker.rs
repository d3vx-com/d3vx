//! Activity Tracker Implementation
//!
//! Tracks agent activity states: active, idle, stuck, blocked, waiting_input.

use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use super::types::{ActivityConfig, ActivityState, BLOCKED_ERROR_THRESHOLD, TOOL_HISTORY_SIZE};

/// Tracks activity state for a single agent/task
pub struct ActivityTracker {
    pub(crate) state: ActivityState,
    pub(crate) last_activity: Instant,
    pub(crate) last_state_change: Instant,
    pub(crate) tool_call_history: Vec<String>,
    pub(crate) error_count: usize,
    pub(crate) config: ActivityConfig,
}

impl ActivityTracker {
    /// Create a new activity tracker with the given configuration
    pub fn new(config: ActivityConfig) -> Self {
        let now = Instant::now();
        Self {
            state: ActivityState::Ready,
            last_activity: now,
            last_state_change: now,
            tool_call_history: Vec::with_capacity(TOOL_HISTORY_SIZE),
            error_count: 0,
            config,
        }
    }

    /// Record a tool call event
    pub fn record_tool_call(&mut self, tool_name: &str) {
        self.last_activity = Instant::now();
        self.tool_call_history.push(tool_name.to_string());
        if self.tool_call_history.len() > TOOL_HISTORY_SIZE {
            self.tool_call_history.remove(0);
        }
        if self.state == ActivityState::Ready || self.state == ActivityState::Idle {
            self.transition(ActivityState::Active);
        }
        debug!(tool = %tool_name, "Recorded tool call");
    }

    /// Record a generation/output event
    pub fn record_output(&mut self) {
        self.last_activity = Instant::now();
        self.error_count = 0;
        if self.state == ActivityState::Ready || self.state == ActivityState::Idle {
            self.transition(ActivityState::Active);
        }
        debug!("Recorded output event");
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
        self.last_activity = Instant::now();
        if self.error_count >= BLOCKED_ERROR_THRESHOLD {
            self.transition(ActivityState::Blocked);
            warn!(
                error_count = self.error_count,
                "Agent blocked due to excessive errors"
            );
        } else {
            debug!(error_count = self.error_count, "Recorded error");
        }
    }

    /// Record that agent is waiting for user input
    pub fn record_waiting_input(&mut self) {
        self.last_activity = Instant::now();
        self.transition(ActivityState::WaitingInput);
        debug!("Agent waiting for user input");
    }

    /// Record that agent exited
    pub fn record_exit(&mut self) {
        self.transition(ActivityState::Exited);
        info!("Agent exited");
    }

    /// Check current state, transitioning if thresholds exceeded
    pub fn check_state(&mut self) -> ActivityState {
        // Terminal states are not automatically transitioned away from
        if matches!(
            self.state,
            ActivityState::Exited | ActivityState::Blocked | ActivityState::WaitingInput
        ) {
            return self.state;
        }

        let idle = self.idle_duration();

        if idle > self.config.stuck_threshold || self.detect_stuck() {
            self.transition(ActivityState::Stuck);
        } else if idle > self.config.idle_threshold {
            self.transition(ActivityState::Idle);
        }

        self.state
    }

    /// Get current state without checking thresholds
    pub fn state(&self) -> ActivityState {
        self.state
    }

    /// Time since last activity
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Detect if agent is stuck by checking repeated tool call patterns.
    ///
    /// Looks at the last N tool calls and checks if they form a repeating
    /// subsequence of length >= 2 that repeats at least `stuck_repeat_threshold`
    /// times. For example: ["edit","bash","read","edit","bash","read"] with
    /// pattern length 3 and repeat count 2.
    pub(crate) fn detect_stuck(&self) -> bool {
        let history = &self.tool_call_history;
        if history.len() < 2 {
            return false;
        }

        // Try pattern lengths from 1 to half the history
        let max_pattern_len = history.len() / 2;
        for pattern_len in 1..=max_pattern_len {
            let min_repeats = self.config.stuck_repeat_threshold;
            if history.len() < pattern_len * min_repeats {
                continue;
            }

            // Check if the tail of history is repetitions of a pattern
            let total_needed = pattern_len * min_repeats;
            let start = history.len().saturating_sub(total_needed);
            let tail = &history[start..];

            let pattern = &tail[..pattern_len];
            let mut repeats = 0;
            let mut all_match = true;

            for chunk in tail.chunks(pattern_len) {
                if chunk == pattern {
                    repeats += 1;
                } else {
                    all_match = false;
                    break;
                }
            }

            if all_match && repeats >= min_repeats {
                debug!(
                    pattern_len = pattern_len,
                    repeats = repeats,
                    "Detected stuck pattern"
                );
                return true;
            }
        }

        false
    }

    /// Transition to new state, logging the change
    pub(crate) fn transition(&mut self, new_state: ActivityState) {
        if self.state != new_state {
            let old = self.state;
            self.state = new_state;
            self.last_state_change = Instant::now();
            info!(from = %old, to = %new_state, "Activity state changed");
        }
    }

    /// Reset error count (e.g., after a successful operation)
    pub fn reset_errors(&mut self) {
        self.error_count = 0;
    }

    /// Get the error count
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Get time since last state change
    pub fn time_in_state(&self) -> Duration {
        self.last_state_change.elapsed()
    }
}
