//! Diff View Widget
//!
//! Renders unified diff output with syntax highlighting for
//! additions and removals.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::ui::theme::Theme;

/// A line in the diff
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub line_type: DiffLineType,
    pub line_number: Option<usize>,
}

/// Type of diff line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
    Header,
    HunkHeader,
}

/// Diff view state
#[derive(Debug, Clone, Default)]
pub struct DiffView {
    /// File path being diffed
    pub file_path: String,
    /// Raw diff content
    pub diff_content: String,
    /// Parsed diff lines
    pub lines: Vec<DiffLine>,
    /// Scroll offset
    pub scroll_offset: usize,
}

impl DiffView {
    /// Create a new diff view from raw diff content
    pub fn new(file_path: &str, diff_content: &str) -> Self {
        let lines = Self::parse_diff(diff_content);
        Self {
            file_path: file_path.to_string(),
            diff_content: diff_content.to_string(),
            lines,
            scroll_offset: 0,
        }
    }

    /// Parse unified diff format
    fn parse_diff(content: &str) -> Vec<DiffLine> {
        let mut lines = Vec::new();
        let mut line_number = 0;

        for line in content.lines() {
            let diff_line = if line.starts_with("+++") || line.starts_with("---") {
                DiffLine {
                    content: line.to_string(),
                    line_type: DiffLineType::Header,
                    line_number: None,
                }
            } else if line.starts_with("@@") {
                DiffLine {
                    content: line.to_string(),
                    line_type: DiffLineType::HunkHeader,
                    line_number: None,
                }
            } else if line.starts_with('+') {
                line_number += 1;
                DiffLine {
                    content: line.to_string(),
                    line_type: DiffLineType::Added,
                    line_number: Some(line_number),
                }
            } else if line.starts_with('-') {
                DiffLine {
                    content: line.to_string(),
                    line_type: DiffLineType::Removed,
                    line_number: None,
                }
            } else {
                line_number += 1;
                DiffLine {
                    content: line.to_string(),
                    line_type: DiffLineType::Context,
                    line_number: Some(line_number),
                }
            };
            lines.push(diff_line);
        }

        lines
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize, viewport_height: usize) {
        let max_scroll = self.lines.len().saturating_sub(viewport_height);
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
    }

    /// Get visible lines for rendering
    pub fn visible_lines(&self, height: usize) -> &[DiffLine] {
        let end = (self.scroll_offset + height).min(self.lines.len());
        &self.lines[self.scroll_offset..end]
    }

    /// Render the diff view
    pub fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
        let height = area.height as usize;

        // Build lines for display
        let display_lines: Vec<Line> = self
            .visible_lines(height.saturating_sub(2)) // Account for borders
            .iter()
            .map(|line| {
                let (prefix, style) = match line.line_type {
                    DiffLineType::Added => (
                        "+",
                        Style::default()
                            .fg(theme.diff.added_text)
                            .bg(theme.diff.added),
                    ),
                    DiffLineType::Removed => (
                        "-",
                        Style::default()
                            .fg(theme.diff.removed_text)
                            .bg(theme.diff.removed),
                    ),
                    DiffLineType::Header => (
                        "",
                        Style::default()
                            .fg(theme.ui.text_muted)
                            .add_modifier(Modifier::BOLD),
                    ),
                    DiffLineType::HunkHeader => (
                        "",
                        Style::default()
                            .fg(theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    DiffLineType::Context => (" ", Style::default()),
                };

                let content = &line.content;
                Line::from(vec![Span::styled(format!("{}{}", prefix, content), style)])
            })
            .collect();

        let block = Block::default()
            .title(format!(" Diff: {} ", self.file_path))
            // Right-aligned dismissal hint — prevents the "I opened this,
            // now how do I close it?" UX dead end for first-time users.
            .title_top(
                ratatui::text::Line::from(" Esc · Ctrl+D to close ")
                    .style(Style::default().fg(theme.ui.text_dim))
                    .right_aligned(),
            )
            .title_style(
                Style::default()
                    .fg(theme.brand)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.ui.border));

        let paragraph = Paragraph::new(display_lines).block(block);

        f.render_widget(paragraph, area);

        // Render scrollbar if needed
        if self.lines.len() > height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state =
                ScrollbarState::new(self.lines.len()).position(self.scroll_offset);

            f.render_stateful_widget(
                scrollbar,
                ratatui::layout::Rect {
                    x: area.x + area.width - 1,
                    y: area.y + 1,
                    width: 1,
                    height: area.height.saturating_sub(2),
                },
                &mut scrollbar_state,
            );
        }
    }
}
