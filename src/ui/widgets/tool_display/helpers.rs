//! Helper functions for tool display

use crate::ipc::ToolStatus;
use crate::ui::symbols::STATUS;
use crate::ui::theme::Theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// Format JSON value for display (with syntax highlighting if possible)
pub fn format_json_for_display(value: &serde_json::Value, max_length: usize) -> String {
    let formatted = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());

    if formatted.len() > max_length {
        format!("{}...", &formatted[..max_length.saturating_sub(3)])
    } else {
        formatted
    }
}

/// Render a compact tool summary line
pub fn render_tool_summary<'a>(
    _name: &'a str,
    _status: ToolStatus,
    count: usize,
    completed: usize,
    theme: &Theme,
) -> Line<'a> {
    let status_icon = if completed == count {
        STATUS.success
    } else {
        STATUS.running
    };

    let color = if completed == count {
        theme.state.success
    } else {
        theme.state.pending
    };

    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(status_icon, Style::default().fg(color)),
        Span::styled(
            format!(" {}/{} tools completed", completed, count),
            Style::default().fg(color),
        ),
    ])
}
