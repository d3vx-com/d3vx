//! Doom Loop Detector tests

use crate::agent::doom_loop::{DoomLoopDetector, ToolCallPattern};
use serde_json::json;

#[test]
fn test_pattern_equality() {
    let input1 = json!({"path": "test.txt", "limit": 100});
    let input2 = json!({"path": "test.txt", "limit": 100});
    let input3 = json!({"path": "other.txt", "limit": 100});

    let p1 = ToolCallPattern::new("Read", &input1);
    let p2 = ToolCallPattern::new("Read", &input2);
    let p3 = ToolCallPattern::new("Read", &input3);

    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

#[test]
fn test_pattern_different_tools() {
    let input = json!({"path": "test.txt"});
    let p1 = ToolCallPattern::new("Read", &input);
    let p2 = ToolCallPattern::new("Bash", &input);
    assert_ne!(p1, p2);
}

#[test]
fn test_pattern_same_tool_different_inputs() {
    let input1 = json!({"file": "a.txt"});
    let input2 = json!({"file": "b.txt"});
    let p1 = ToolCallPattern::new("Read", &input1);
    let p2 = ToolCallPattern::new("Read", &input2);
    assert_ne!(p1, p2);
}

#[test]
fn test_pattern_empty_input() {
    let input = json!({});
    let p = ToolCallPattern::new("Bash", &input);
    assert_eq!(p.tool_name, "Bash");
}

#[test]
fn test_doom_loop_detection() {
    let mut detector = DoomLoopDetector::with_config(3, 10);
    let input = json!({"path": "test.txt"});

    assert!(detector.record("Read", &input).is_none());
    assert!(detector.record("Read", &input).is_none());

    let warning = detector.record("Read", &input);
    assert!(warning.is_some());
    let warning = warning.unwrap();
    assert_eq!(warning.tool, "Read");
    assert_eq!(warning.repeats, 3);
    assert!(!warning.suggestion.is_empty());

    assert!(detector.record("Read", &input).is_none());
}

#[test]
fn test_different_inputs_no_loop() {
    let mut detector = DoomLoopDetector::with_config(3, 10);

    for i in 0..5 {
        let input = json!({"path": format!("test{}.txt", i)});
        assert!(detector.record("Read", &input).is_none());
    }

    assert_eq!(detector.statistics().unique_patterns, 5);
}

#[test]
fn test_reset() {
    let mut detector = DoomLoopDetector::with_config(2, 10);
    let input = json!({"path": "test.txt"});

    detector.record("Read", &input);
    detector.record("Read", &input);
    assert!(detector.history_size() != 0);

    detector.reset();
    assert_eq!(detector.history_size(), 0);
    assert_eq!(detector.statistics().total_tool_calls, 0);
    assert_eq!(detector.statistics().unique_patterns, 0);
    assert_eq!(detector.statistics().loop_warnings, 0);
}

#[test]
fn test_history_size() {
    let mut detector = DoomLoopDetector::with_config(3, 3);

    for i in 0..5 {
        let input = json!({"path": format!("test{}.txt", i)});
        detector.record("Read", &input);
    }

    assert_eq!(detector.history_size(), 3);
}

#[test]
fn test_warning_has_suggestion() {
    let mut detector = DoomLoopDetector::with_config(2, 5);
    let input = json!({"path": "test.txt"});

    detector.record("Read", &input);
    let warning = detector.record("Read", &input);

    assert!(warning.is_some());
    let w = warning.unwrap();
    assert!(!w.suggestion.is_empty());
    assert!(!w.patterns.is_empty());
    assert_eq!(w.patterns.len(), 2);
}

#[test]
fn test_statistics_tracking() {
    let mut detector = DoomLoopDetector::with_config(3, 10);
    let input = json!({"cmd": "ls"});

    detector.record("Bash", &input);
    detector.record("Bash", &input);
    detector.record("Bash", &input);

    let stats = detector.statistics();
    assert_eq!(stats.total_tool_calls, 3);
    assert_eq!(stats.unique_patterns, 1);
    assert_eq!(stats.loop_warnings, 1);
}

#[test]
fn test_default_detector() {
    let detector = DoomLoopDetector::default();
    assert_eq!(detector.history_size(), 0);
    assert_eq!(detector.statistics().total_tool_calls, 0);
}

#[test]
fn test_with_custom_config() {
    let detector = DoomLoopDetector::with_config(5, 20);
    assert_eq!(detector.history_size(), 0);
}
