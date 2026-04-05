//! Help Modal Widget
//!
//! Minimal popup showing essential keyboard shortcuts.

use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// A help modal that displays essential keybindings and slash commands.
pub struct HelpModal {
    theme: Theme,
    scroll: usize,
}

impl HelpModal {
    pub fn new(theme: Theme, scroll: usize) -> Self {
        Self { theme, scroll }
    }

    /// Helper to create centered rect of relative size
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

impl Widget for HelpModal {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let popup_area = self.centered_rect(70, 70, area);
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(Span::styled(
                " Help & Commands ",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.brand))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let mut text = Vec::new();

        // 1. Keyboard Shortcuts
        text.push(Line::from(vec![
            Span::styled(
                "─ Keyboard Shortcuts ",
                Style::default().fg(self.theme.brand),
            ),
            Span::styled(
                "──────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        let shortcuts = vec![
            ("Enter", "Send message"),
            ("\\ + Enter", "New line"),
            ("↑ / ↓", "History navigation"),
            ("Ctrl+C", "Cancel / Quit"),
            ("Ctrl+P", "Power Mode (Advanced stats)"),
            ("Ctrl+L", "Toggle sidebar"),
            ("Ctrl+O", "Toggle tools (selected agent)"),
            ("Esc", "Dismiss modal / Stop agent"),
            ("?", "Quick help"),
        ];

        for (key, desc) in shortcuts {
            text.push(Line::from(vec![
                Span::styled(
                    format!("  {:>14} ", key),
                    Style::default()
                        .fg(self.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(self.theme.ui.text_dim)),
            ]));
        }

        text.push(Line::from(""));

        // 2. Slash Commands
        text.push(Line::from(vec![
            Span::styled("─ Slash Commands ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "─────────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        use crate::app::slash_commands::SLASH_COMMANDS;
        for cmd in SLASH_COMMANDS {
            text.push(Line::from(vec![
                Span::styled(
                    format!("  /{:<10} ", cmd.name),
                    Style::default()
                        .fg(self.theme.brand_secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(cmd.description, Style::default().fg(self.theme.ui.text_dim)),
            ]));
            text.push(Line::from(vec![Span::styled(
                format!("               usage: {}", cmd.usage),
                Style::default()
                    .fg(self.theme.ui.text_muted)
                    .add_modifier(Modifier::ITALIC),
            )]));
        }

        text.push(Line::from(""));
        text.push(Line::from(vec![Span::styled(
            "  Use ↑/↓ or j/k to scroll. Esc to close.",
            Style::default().fg(self.theme.ui.text_muted),
        )]));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .scroll((self.scroll as u16, 0));

        paragraph.render(popup_area, buf);
    }
}
