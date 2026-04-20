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

        // 0. Getting Started
        text.push(Line::from(vec![
            Span::styled("─ Getting Started ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "─────────────────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        let steps = [
            ("1.", "Describe what you want done"),
            ("2.", "Review the result in the conversation"),
            ("3.", "Approve changes or ask for adjustments"),
        ];
        for (num, desc) in steps {
            text.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", num),
                    Style::default()
                        .fg(self.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(self.theme.ui.text)),
            ]));
        }

        text.push(Line::from(vec![
            Span::styled("  Add ", Style::default().fg(self.theme.ui.text_dim)),
            Span::styled(
                "--vex ",
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "to run a task in an isolated worktree.",
                Style::default().fg(self.theme.ui.text_dim),
            ),
        ]));

        text.push(Line::from(""));

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

        // Grouped by *where you'd use them* so a user can scan the
        // block that matches their current context rather than
        // memorise a flat list of 20 bindings.
        let shortcut_groups: &[(&str, &[(&str, &str)])] = &[
            (
                "Chat",
                &[
                    ("Enter", "Send message"),
                    ("\\ + Enter", "New line within a message"),
                    ("↑ / ↓", "History (with prefix search)"),
                    ("Esc", "Stop streaming / close modals / clear"),
                    ("Ctrl+C", "Interrupt · press twice to quit"),
                ],
            ),
            (
                "Slash palette",
                &[
                    ("/", "Open the live command palette"),
                    ("↑ / ↓", "Navigate palette while open"),
                    ("Tab", "Complete the highlighted command"),
                    ("Enter", "Accept and run the highlighted command"),
                ],
            ),
            (
                "Views",
                &[
                    ("Ctrl+1..4", "Switch right-panel tab"),
                    ("Ctrl+L", "Toggle left sidebar"),
                    ("Ctrl+W", "Toggle detail drawer"),
                    ("Ctrl+O", "Expand/collapse selected tool output"),
                    ("Ctrl+F", "Cycle focus mode"),
                    ("?", "Quick help · Esc to close"),
                ],
            ),
        ];

        for (group_title, bindings) in shortcut_groups {
            text.push(Line::from(vec![Span::styled(
                format!("  {}", group_title),
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            for (key, desc) in *bindings {
                text.push(Line::from(vec![
                    Span::styled(
                        format!("  {:>14} ", key),
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
            text.push(Line::raw(""));
        }

        // 2. Slash Commands — grouped by category
        text.push(Line::from(vec![
            Span::styled("─ Slash Commands ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "─────────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        use crate::app::slash_commands::{CATEGORY_ORDER, SLASH_COMMANDS};
        for category in CATEGORY_ORDER {
            let matching: Vec<_> = SLASH_COMMANDS
                .iter()
                .filter(|c| c.category == *category)
                .collect();
            if matching.is_empty() {
                continue;
            }

            text.push(Line::from(vec![Span::styled(
                format!("  {}", category),
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));

            for cmd in matching {
                text.push(Line::from(vec![
                    Span::styled(
                        format!("  /{:<11} ", cmd.name),
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(cmd.description, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
            text.push(Line::raw(""));
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
