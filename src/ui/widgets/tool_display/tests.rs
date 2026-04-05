//! Tests for tool display widget

use super::helpers::{format_json_for_display, render_tool_summary};
use super::types::{ToolDisplay, ToolDisplayConfig};
use crate::ipc::ToolStatus;
use crate::ui::theme::Theme;

#[test]
fn test_tool_display_pending() {
    let input = serde_json::json!({"command": "ls -la"});
    let display = ToolDisplay::new("BashTool", "tool_123", &input, ToolStatus::Pending);
    let lines = display.build_lines();

    assert!(!lines.is_empty());
    assert!(lines[0]
        .spans
        .iter()
        .any(|s| s.content.contains("BashTool")));
}

#[test]
fn test_tool_display_completed() {
    let input = serde_json::json!({"file_path": "/test.txt"});
    let display = ToolDisplay::new("ReadTool", "tool_456", &input, ToolStatus::Completed)
        .output(Some("File contents here"))
        .elapsed(150);
    let lines = display.build_lines();

    assert!(!lines.is_empty());
    assert!(lines[0]
        .spans
        .iter()
        .any(|s| s.content.contains("ReadTool")));
}

#[test]
fn test_tool_display_error() {
    let input = serde_json::json!({"file_path": "/nonexistent.txt"});
    let display = ToolDisplay::new("ReadTool", "tool_789", &input, ToolStatus::Error)
        .output(Some("File not found"))
        .config(ToolDisplayConfig {
            verbose: true,
            ..Default::default()
        });
    let lines = display.build_lines();

    assert!(!lines.is_empty());
    assert!(lines
        .iter()
        .any(|l| l.spans.iter().any(|s| s.content.contains("File not found"))));
}

#[test]
fn test_format_json_for_display() {
    let value = serde_json::json!({"key": "value"});
    let formatted = format_json_for_display(&value, 100);

    assert!(formatted.contains("key"));
    assert!(formatted.contains("value"));
}

#[test]
fn test_render_tool_summary() {
    let theme = Theme::dark();
    let line = render_tool_summary("BashTool", ToolStatus::Completed, 5, 3, &theme);

    assert!(line.spans.iter().any(|s| s.content.contains("3/5")));
}
