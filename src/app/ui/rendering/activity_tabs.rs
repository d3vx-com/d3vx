//! Activity panel tabs and main session detail rendering

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ui::icons;

impl App {
    /// Render the main session detail in the activity inspector
    pub(super) fn render_main_session_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        bg_color: Color,
    ) {
        let mut detail_lines: Vec<Line<'static>> = Vec::new();
        detail_lines.push(Line::from(vec![Span::styled(
            "Main Session Activity",
            Style::default()
                .fg(self.ui.theme.brand)
                .add_modifier(Modifier::BOLD),
        )]));
        detail_lines.push(Line::from(vec![Span::styled(
            "Recent tool calls and status updates from the primary chat session.",
            Style::default().fg(self.ui.theme.ui.text_dim),
        )]));
        detail_lines.push(Line::raw(""));

        if !self.tools.executing_tools.is_empty() {
            detail_lines.push(Line::from(vec![Span::styled(
                "Active Tools",
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            for tool in &self.tools.executing_tools {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!(" {} ", icons::status::info()),
                        Style::default().fg(Color::Rgb(100, 200, 255)),
                    ),
                    Span::styled(
                        tool.name.clone(),
                        Style::default().fg(self.ui.theme.ui.text),
                    ),
                ]));
            }
            detail_lines.push(Line::raw(""));
        }

        if !self.tools.recent_tools.is_empty() {
            detail_lines.push(Line::from(vec![Span::styled(
                "Recent Tools",
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            for tool in self.tools.recent_tools.iter().rev().take(10) {
                let color = if tool.is_error {
                    Color::Rgb(255, 100, 100)
                } else {
                    Color::Rgb(100, 255, 150)
                };
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!(
                            " {} ",
                            if tool.is_error {
                                icons::status::x()
                            } else {
                                icons::status::check()
                            }
                        ),
                        Style::default().fg(color),
                    ),
                    Span::styled(
                        tool.name.clone(),
                        Style::default().fg(self.ui.theme.ui.text),
                    ),
                    Span::styled(
                        format!(" ({}ms)", tool.elapsed_ms),
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                ]));
            }
        } else if self.tools.executing_tools.is_empty() {
            detail_lines.push(Line::from(vec![Span::styled(
                "No recent activity found.",
                Style::default().fg(self.ui.theme.ui.text_dim),
            )]));
        }

        self.ui.selected_agent_output_lines = detail_lines.len();
        let paragraph = Paragraph::new(Text::from(detail_lines))
            .style(Style::default().bg(bg_color))
            .wrap(Wrap { trim: true })
            .scroll((self.ui.selected_agent_output_scroll as u16, 0));
        f.render_widget(paragraph, area);
    }

    /// Render the right pane tab bar (Agent / Diff / Batch / Trust)
    pub(super) fn render_right_pane_tabs(&self, f: &mut Frame, area: Rect, bg_color: Color) {
        let sep_color = Color::Rgb(40, 40, 50);
        let active_bg = Color::Rgb(35, 35, 50);
        let inactive_bg = Color::Rgb(28, 28, 38);

        let tabs = [
            ("1", "Agent", crate::app::state::RightPaneTab::Agent),
            ("2", "Diff", crate::app::state::RightPaneTab::Diff),
            ("3", "Batch", crate::app::state::RightPaneTab::Batch),
            ("4", "Readiness", crate::app::state::RightPaneTab::Trust),
        ];

        let mut spans: Vec<Span<'_>> = Vec::new();

        for (i, (key, label, tab_idx)) in tabs.iter().enumerate() {
            let is_active = self.selected_right_pane_tab == *tab_idx;

            if i > 0 {
                spans.push(Span::styled(
                    "│",
                    Style::default().fg(sep_color).bg(bg_color),
                ));
            }

            if is_active {
                spans.push(Span::styled(
                    format!(" {}:{} ", key, label),
                    Style::default()
                        .fg(self.ui.theme.ui.text)
                        .bg(active_bg)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                // Inactive tab with subtle background
                spans.push(Span::styled(
                    format!(" {}:{} ", key, label),
                    Style::default()
                        .fg(self.ui.theme.ui.text_dim)
                        .bg(inactive_bg),
                ));
            }
        }

        spans.push(Span::styled(
            "  Ctrl+Left/Right cycle · Ctrl+D full",
            Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
        ));

        let paragraph = Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(sep_color))
                    .style(Style::default().bg(bg_color)),
            )
            .style(Style::default().bg(bg_color));
        f.render_widget(paragraph, area);
    }
}
