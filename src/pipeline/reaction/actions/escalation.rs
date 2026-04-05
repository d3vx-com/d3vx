//! Escalation policies and tracking for reaction actions.
//!
//! Tracks recurring issues and determines when a situation exceeds
//! configured retry/time budgets and must be escalated.

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ============================================================================
// ESCALATION ACTION
// ============================================================================

/// Actions that can be taken when escalation triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EscalationAction {
    /// Notify a human through the given channels (e.g. "slack", "email").
    NotifyHuman {
        /// Channel names to notify through.
        channels: Vec<String>,
    },
    /// Kill the session immediately.
    KillSession,
    /// Restart the session from scratch.
    RestartSession,
    /// Switch to a different model and retry.
    ChangeModel {
        /// Model identifier to switch to.
        model: String,
    },
}

// ============================================================================
// ESCALATION POLICY
// ============================================================================

/// Policy governing when escalation should occur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPolicy {
    /// Maximum retries before escalation (default 2).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Seconds after first occurrence before escalation (default 1800 = 30 min).
    #[serde(default = "default_escalate_after_secs")]
    pub escalate_after_secs: u64,
    /// Actions to execute on escalation.
    #[serde(default)]
    pub actions: Vec<EscalationAction>,
}

impl Default for EscalationPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            escalate_after_secs: default_escalate_after_secs(),
            actions: Vec::new(),
        }
    }
}

fn default_max_retries() -> u32 {
    2
}
fn default_escalate_after_secs() -> u64 {
    1800
}

// ============================================================================
// ESCALATION STATUS
// ============================================================================

/// Status returned when recording an event in the tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationStatus {
    /// First time this issue has been seen.
    FirstOccurrence,
    /// Still within budget.
    WithinBudget {
        /// Retries remaining before escalation.
        retries_left: u32,
        /// Seconds remaining before time-based escalation.
        time_left_secs: u64,
    },
    /// Budget exhausted, must escalate now.
    NeedsEscalation,
}

// ============================================================================
// ESCALATION TRACKER
// ============================================================================

/// Tracks recurring issues and decides when to escalate.
#[derive(Debug)]
pub struct EscalationTracker {
    /// Retry counts keyed by "session_id:event_type".
    retry_counts: HashMap<String, u32>,
    /// When each issue was first seen.
    first_seen: HashMap<String, Instant>,
    /// Whether escalation already fired for a given key.
    escalated: HashMap<String, bool>,
}

/// Maximum age of entries before cleanup removes them (1 hour).
const CLEANUP_SECS: u64 = 3600;

impl EscalationTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            retry_counts: HashMap::new(),
            first_seen: HashMap::new(),
            escalated: HashMap::new(),
        }
    }

    /// Build the composite key from session ID and event type.
    fn key(session_id: &str, event_type: &str) -> String {
        format!("{}:{}", session_id, event_type)
    }

    /// Record an event occurrence and return the current status.
    pub fn record_event(&mut self, session_id: &str, event_type: &str) -> EscalationStatus {
        let key = Self::key(session_id, event_type);

        let count = self.retry_counts.entry(key.clone()).or_insert(0);
        *count += 1;
        self.first_seen
            .entry(key.clone())
            .or_insert_with(Instant::now);

        // Already escalated -- keep signalling.
        if self.escalated.get(&key).copied().unwrap_or(false) {
            return EscalationStatus::NeedsEscalation;
        }

        if *count == 1 {
            return EscalationStatus::FirstOccurrence;
        }

        // Defer time calculation to caller via should_escalate; here we
        // return WithinBudget with placeholder. The caller should call
        // should_escalate next to get the definitive answer.
        EscalationStatus::WithinBudget {
            retries_left: 0,
            time_left_secs: 0,
        }
    }

    /// Determine whether the issue should be escalated given a policy.
    pub fn should_escalate(
        &self,
        session_id: &str,
        event_type: &str,
        policy: &EscalationPolicy,
    ) -> bool {
        let key = Self::key(session_id, event_type);

        // Already escalated.
        if self.escalated.get(&key).copied().unwrap_or(false) {
            return true;
        }

        let count = self.retry_counts.get(&key).copied().unwrap_or(0);
        if count > policy.max_retries {
            return true;
        }

        if let Some(first) = self.first_seen.get(&key) {
            let elapsed = first.elapsed().as_secs();
            if elapsed >= policy.escalate_after_secs {
                return true;
            }
        }

        false
    }

    /// Full resolution: record + check in one call.
    ///
    /// Records the event, checks against policy, marks escalated if needed,
    /// and returns the definitive status.
    pub fn record_and_check(
        &mut self,
        session_id: &str,
        event_type: &str,
        policy: &EscalationPolicy,
    ) -> EscalationStatus {
        let key = Self::key(session_id, event_type);

        // Record.
        let count = self.retry_counts.entry(key.clone()).or_insert(0);
        *count += 1;
        self.first_seen
            .entry(key.clone())
            .or_insert_with(Instant::now);

        // Already escalated.
        if self.escalated.get(&key).copied().unwrap_or(false) {
            return EscalationStatus::NeedsEscalation;
        }

        let retries_left = policy.max_retries.saturating_sub(*count);
        let time_left_secs = self
            .first_seen
            .get(&key)
            .map(|first| {
                let elapsed = first.elapsed().as_secs();
                policy.escalate_after_secs.saturating_sub(elapsed)
            })
            .unwrap_or(policy.escalate_after_secs);

        let over_budget = *count > policy.max_retries || time_left_secs == 0;

        if over_budget {
            self.escalated.insert(key, true);
            return EscalationStatus::NeedsEscalation;
        }

        if *count == 1 {
            EscalationStatus::FirstOccurrence
        } else {
            EscalationStatus::WithinBudget {
                retries_left,
                time_left_secs,
            }
        }
    }

    /// Reset tracking for a resolved issue.
    pub fn reset(&mut self, session_id: &str, event_type: &str) {
        let key = Self::key(session_id, event_type);
        self.retry_counts.remove(&key);
        self.first_seen.remove(&key);
        self.escalated.remove(&key);
    }

    /// Remove entries older than the cleanup threshold.
    pub fn cleanup(&mut self) {
        let cutoff = Instant::now() - std::time::Duration::from_secs(CLEANUP_SECS);
        self.first_seen.retain(|key, instant| {
            if *instant < cutoff {
                self.retry_counts.remove(key);
                self.escalated.remove(key);
                false
            } else {
                true
            }
        });
    }
}

impl Default for EscalationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_occurrence() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy::default();
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::FirstOccurrence);
    }

    #[test]
    fn test_within_budget() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 3,
            escalate_after_secs: 1800,
            actions: Vec::new(),
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        // Second occurrence, count=2, budget = 3, so retries_left = 1.
        match status {
            EscalationStatus::WithinBudget { retries_left, .. } => {
                assert_eq!(retries_left, 1);
            }
            other => panic!("Expected WithinBudget, got {:?}", other),
        }
    }

    #[test]
    fn test_needs_escalation_by_retries() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 1,
            escalate_after_secs: 99999,
            actions: vec![EscalationAction::KillSession],
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::NeedsEscalation);
    }

    #[test]
    fn test_should_escalate_reflects_retries() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 2,
            escalate_after_secs: 99999,
            actions: Vec::new(),
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert!(!tracker.should_escalate("sess-1", "ci_failure", &policy));
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert!(!tracker.should_escalate("sess-1", "ci_failure", &policy));
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert!(tracker.should_escalate("sess-1", "ci_failure", &policy));
    }

    #[test]
    fn test_should_escalate_by_time() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 999,
            escalate_after_secs: 0, // immediate time trigger
            actions: Vec::new(),
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        // With escalate_after_secs = 0, the very first check will have
        // time_left_secs = 0 which triggers escalation.
        // We need to check on the *next* call because first occurrence
        // returns FirstOccurrence regardless of time.
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::NeedsEscalation);
    }

    #[test]
    fn test_reset_clears_tracking() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy::default();
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        tracker.reset("sess-1", "ci_failure");
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::FirstOccurrence);
    }

    #[test]
    fn test_cleanup_removes_old_entries() {
        let mut tracker = EscalationTracker::new();
        // Manually insert an old entry.
        let key = EscalationTracker::key("old-sess", "ci_failure");
        tracker.retry_counts.insert(key.clone(), 5);
        tracker.first_seen.insert(
            key.clone(),
            Instant::now() - std::time::Duration::from_secs(7200),
        );
        tracker.escalated.insert(key.clone(), true);

        tracker.cleanup();

        assert!(tracker.retry_counts.is_empty());
        assert!(tracker.first_seen.is_empty());
        assert!(tracker.escalated.is_empty());
    }

    #[test]
    fn test_cleanup_keeps_recent_entries() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy::default();
        tracker.record_and_check("recent-sess", "ci_failure", &policy);

        tracker.cleanup();

        assert_eq!(tracker.retry_counts.len(), 1);
        assert_eq!(tracker.first_seen.len(), 1);
    }

    #[test]
    fn test_escalation_persists() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 0,
            escalate_after_secs: 99999,
            actions: Vec::new(),
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        // count=1 > max_retries=0, so NeedsEscalation.
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::NeedsEscalation);
        // Subsequent calls should still return NeedsEscalation.
        let status = tracker.record_and_check("sess-1", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::NeedsEscalation);
    }

    #[test]
    fn test_different_sessions_independent() {
        let mut tracker = EscalationTracker::new();
        let policy = EscalationPolicy {
            max_retries: 1,
            escalate_after_secs: 99999,
            actions: Vec::new(),
        };
        tracker.record_and_check("sess-1", "ci_failure", &policy);
        let status = tracker.record_and_check("sess-2", "ci_failure", &policy);
        assert_eq!(status, EscalationStatus::FirstOccurrence);
    }

    #[test]
    fn test_default_policy() {
        let policy = EscalationPolicy::default();
        assert_eq!(policy.max_retries, 2);
        assert_eq!(policy.escalate_after_secs, 1800);
        assert!(policy.actions.is_empty());
    }
}
