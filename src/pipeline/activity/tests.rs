//! Activity Detection Tests

use super::{
    ActivityConfig, ActivityState, ActivityTracker, BLOCKED_ERROR_THRESHOLD, TOOL_HISTORY_SIZE,
};
use std::time::Duration;

#[test]
fn test_activity_config_default() {
    let config = ActivityConfig::default();
    assert_eq!(config.idle_threshold, Duration::from_secs(120));
    assert_eq!(config.stuck_threshold, Duration::from_secs(300));
    assert_eq!(config.stuck_repeat_threshold, 3);
}

#[test]
fn test_tracker_starts_ready() {
    let tracker = ActivityTracker::new(ActivityConfig::default());
    assert_eq!(tracker.state(), ActivityState::Ready);
}

#[test]
fn test_record_tool_call_transitions_to_active() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    assert_eq!(tracker.state(), ActivityState::Ready);
    tracker.record_tool_call("bash");
    assert_eq!(tracker.state(), ActivityState::Active);
}

#[test]
fn test_record_output_transitions_ready_to_active() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_output();
    assert_eq!(tracker.state(), ActivityState::Active);
}

#[test]
fn test_record_error_increments_count() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    for _ in 0..4 {
        tracker.record_error();
    }
    assert_eq!(tracker.error_count(), 4);
    assert_eq!(tracker.state(), ActivityState::Ready);
}

#[test]
fn test_record_error_transitions_to_blocked() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    for _ in 0..BLOCKED_ERROR_THRESHOLD {
        tracker.record_error();
    }
    assert_eq!(tracker.state(), ActivityState::Blocked);
}

#[test]
fn test_record_waiting_input() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_tool_call("bash");
    tracker.record_waiting_input();
    assert_eq!(tracker.state(), ActivityState::WaitingInput);
}

#[test]
fn test_record_exit() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_tool_call("bash");
    tracker.record_exit();
    assert_eq!(tracker.state(), ActivityState::Exited);
}

#[test]
fn test_terminal_states_not_auto_transitioned() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_exit();
    assert_eq!(tracker.check_state(), ActivityState::Exited);

    let mut tracker2 = ActivityTracker::new(ActivityConfig::default());
    for _ in 0..BLOCKED_ERROR_THRESHOLD {
        tracker2.record_error();
    }
    assert_eq!(tracker2.check_state(), ActivityState::Blocked);
}

#[test]
fn test_tool_history_truncated() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    for i in 0..25 {
        tracker.record_tool_call(&format!("tool_{}", i));
    }
    assert!(tracker.tool_call_history.len() <= TOOL_HISTORY_SIZE);
    // Last entry should be the most recent
    assert_eq!(
        tracker.tool_call_history.last().map(String::as_str),
        Some("tool_24")
    );
}

#[test]
fn test_detect_stuck_simple_repeat() {
    let mut tracker = ActivityTracker::new(ActivityConfig {
        stuck_repeat_threshold: 3,
        ..Default::default()
    });
    // Pattern: ["edit", "bash", "read"] repeated 3 times = 9 calls
    for _ in 0..3 {
        tracker.record_tool_call("edit");
        tracker.record_tool_call("bash");
        tracker.record_tool_call("read");
    }
    assert!(tracker.detect_stuck());
}

#[test]
fn test_detect_stuck_single_tool_repeat() {
    let mut tracker = ActivityTracker::new(ActivityConfig {
        stuck_repeat_threshold: 3,
        ..Default::default()
    });
    for _ in 0..5 {
        tracker.record_tool_call("glob");
    }
    // 5 calls with pattern_len=1, repeats=5 >= 3
    assert!(tracker.detect_stuck());
}

#[test]
fn test_no_stuck_with_varied_tools() {
    let mut tracker = ActivityTracker::new(ActivityConfig {
        stuck_repeat_threshold: 3,
        ..Default::default()
    });
    // Varied sequence, no repeating pattern
    tracker.record_tool_call("bash");
    tracker.record_tool_call("read");
    tracker.record_tool_call("edit");
    tracker.record_tool_call("grep");
    tracker.record_tool_call("glob");
    tracker.record_tool_call("write");
    assert!(!tracker.detect_stuck());
}

#[test]
fn test_no_stuck_with_short_history() {
    let tracker = ActivityTracker::new(ActivityConfig {
        stuck_repeat_threshold: 3,
        ..Default::default()
    });
    assert!(!tracker.detect_stuck());
}

#[test]
fn test_idle_duration_increases() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_tool_call("bash");
    let before = tracker.idle_duration();
    std::thread::sleep(Duration::from_millis(50));
    let after = tracker.idle_duration();
    assert!(after > before);
}

#[test]
fn test_reset_errors() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.record_error();
    tracker.record_error();
    assert_eq!(tracker.error_count(), 2);
    tracker.reset_errors();
    assert_eq!(tracker.error_count(), 0);
}

#[test]
fn test_time_in_state() {
    let tracker = ActivityTracker::new(ActivityConfig::default());
    std::thread::sleep(Duration::from_millis(50));
    assert!(tracker.time_in_state() >= Duration::from_millis(50));
}

#[test]
fn test_state_display() {
    assert_eq!(ActivityState::Active.to_string(), "active");
    assert_eq!(ActivityState::Ready.to_string(), "ready");
    assert_eq!(ActivityState::Idle.to_string(), "idle");
    assert_eq!(ActivityState::WaitingInput.to_string(), "waiting_input");
    assert_eq!(ActivityState::Blocked.to_string(), "blocked");
    assert_eq!(ActivityState::Stuck.to_string(), "stuck");
    assert_eq!(ActivityState::Exited.to_string(), "exited");
}

#[test]
fn test_detect_stuck_with_noise_prefix() {
    let mut tracker = ActivityTracker::new(ActivityConfig {
        stuck_repeat_threshold: 3,
        ..Default::default()
    });
    // Some noise before the repeating pattern
    tracker.record_tool_call("read");
    tracker.record_tool_call("glob");
    // Pattern repeats 3 times
    for _ in 0..3 {
        tracker.record_tool_call("edit");
        tracker.record_tool_call("bash");
    }
    assert!(tracker.detect_stuck());
}

#[test]
fn test_transition_only_logs_on_change() {
    let mut tracker = ActivityTracker::new(ActivityConfig::default());
    tracker.transition(ActivityState::Active);
    let change_time = tracker.time_in_state();
    std::thread::sleep(Duration::from_millis(10));
    // Same-state transition should not update last_state_change
    tracker.transition(ActivityState::Active);
    // time_in_state should not have been reset
    assert!(tracker.time_in_state() >= change_time);
}
