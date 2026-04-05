//! Shared UI Helper Functions
//!
//! Common utilities for rendering UI components.

use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::ui::symbols::SPINNER_DOTS;
use crate::ui::theme::Theme;

/// Muted white color for running agents
pub const MUTED_WHITE: Color = Color::Rgb(160, 160, 170);

/// Get the current braille spinner frame based on wall-clock time
pub fn braille_frame() -> &'static str {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    // Cycle every ~80ms per frame for smooth animation
    let index = (millis / 80) as usize % SPINNER_DOTS.len();
    SPINNER_DOTS[index]
}

/// Apply standard container styling with title (returns owned Block)
pub fn styled_block(title: &str, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(title.to_string())
        .border_style(Style::default().fg(theme.ui.border))
}

/// Create padded inner area with specified margins
pub fn padded_area(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    area.inner(Margin {
        horizontal,
        vertical,
    })
}

/// Helper function to create a centered rect
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Render a vertical separator line
pub fn render_vertical_separator(f: &mut Frame, x: u16, height: u16, color: Color) {
    let separator_style = Style::default().fg(color);
    for y in 0..height {
        #[allow(deprecated)]
        {
            f.buffer_mut()
                .get_mut(x, y)
                .set_char('│')
                .set_style(separator_style);
        }
    }
}

/// Create a styled header span (returns owned span)
pub fn header_span(text: impl Into<String>, theme: &Theme) -> ratatui::text::Span<'static> {
    ratatui::text::Span::styled(
        text.into(),
        Style::default()
            .fg(theme.brand_secondary)
            .add_modifier(Modifier::BOLD),
    )
}

/// Create a muted text span (returns owned span)
pub fn muted_span(text: impl Into<String>, theme: &Theme) -> ratatui::text::Span<'static> {
    ratatui::text::Span::styled(text.into(), Style::default().fg(theme.ui.text_dim))
}

/// Create a standard text span (returns owned span)
pub fn text_span(text: impl Into<String>, theme: &Theme) -> ratatui::text::Span<'static> {
    ratatui::text::Span::styled(text.into(), Style::default().fg(theme.ui.text))
}

/// Truncate text to max length with ellipsis
pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    } else {
        text.to_string()
    }
}

/// Format elapsed time as human-readable string
pub fn format_elapsed_short(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m {}s", mins, secs)
    }
}
