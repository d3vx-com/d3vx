//! Agent Strip — fleet status pills above the input area

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

use super::activity_tools::inline_agent_icon_color;

impl App {
    /// Render the agent strip — a horizontal bar of clickable agent pills.
    ///
    /// Collapsed (1 row): "◐ 2 active · 3 background"
    /// Expanded (2 rows): individual agent pills with status, task name, elapsed
    pub fn render_agent_strip(&mut self, f: &mut Frame, area: Rect) {
        self.layout.strip_pill_positions.clear();
        f.render_widget(Clear, area);

        let agents = &self.agents.inline_agents;
        if agents.is_empty() {
            return;
        }

        let is_expanded = self.ui.strip_expanded;

        // Count active vs background
        let (active, background) = agents.iter().fold((0, 0), |(a, b), agent| {
            if agent.id.starts_with("vex:") {
                (a, b + 1)
            } else {
                (a + 1, b)
            }
        });

        if !is_expanded {
            // Collapsed: single-line summary
            let mut spans = Vec::new();
            if active > 0 {
                spans.push(Span::styled(
                    format!(" \u{25d0} {} active ", active),
                    Style::default()
                        .fg(Color::Black)
                        .bg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if background > 0 {
                if active > 0 {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(
                    format!(" \u{25d1} {} background ", background),
                    Style::default()
                        .fg(Color::Rgb(140, 140, 155))
                        .bg(Color::Rgb(40, 40, 50)),
                ));
            }
            spans.push(Span::styled(
                "  click to expand",
                Style::default().fg(Color::Rgb(60, 60, 70)),
            ));
            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            // Expanded: individual agent pills
            let mut spans: Vec<Span<'_>> = Vec::new();
            let mut col = area.x;
            let max_width = area.width as usize;

            for (idx, agent) in agents.iter().enumerate() {
                let (icon, color) = inline_agent_icon_color(agent.status, &self.ui.theme);
                let is_selected = self.agents.selected_inline_agent == Some(idx);
                let short_task = truncate_strip_task(&agent.task, 18);
                let pill_text = format!(" {} {} {} ", icon, short_task, agent.elapsed());
                let pill_width = pill_text.len() as u16;

                // Check if pill fits
                if (col - area.x) as usize + pill_text.len() > max_width {
                    break;
                }

                let start_col = col;

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else if agent.id.starts_with("vex:") {
                    // Background agents are dimmer
                    Style::default().fg(color).bg(Color::Rgb(30, 30, 38))
                } else {
                    Style::default().fg(color).bg(Color::Rgb(35, 35, 48))
                };

                self.layout
                    .strip_pill_positions
                    .push((start_col, start_col + pill_width, idx));
                col += pill_width;

                spans.push(Span::styled(pill_text, style));
                spans.push(Span::raw(" "));
                col += 1;
            }

            let line = Line::from(spans);
            f.render_widget(Paragraph::new(line), area);

            // Second row (if expanded) shows hint
            if area.height > 1 {
                let hint = Line::from(Span::styled(
                    " click agent to inspect | Ctrl+W drawer | Ctrl+S collapse",
                    Style::default().fg(Color::Rgb(50, 50, 60)),
                ));
                let row2 = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: 1,
                };
                f.render_widget(Paragraph::new(hint), row2);
            }
        }
    }
}

/// Truncate a task description for the strip pill.
pub(super) fn truncate_strip_task(task: &str, max_chars: usize) -> String {
    if task.len() <= max_chars {
        return task.to_string();
    }
    let mut used = 0;
    let mut words: Vec<&str> = Vec::new();
    for word in task.split_whitespace() {
        let needed = word.len() + if words.is_empty() { 0 } else { 1 };
        if used + needed > max_chars.saturating_sub(2) {
            break;
        }
        words.push(word);
        used += needed;
    }
    if words.is_empty() {
        format!("{}..", &task[..max_chars.saturating_sub(2).max(1)])
    } else {
        format!("{}..", words.join(" "))
    }
}
