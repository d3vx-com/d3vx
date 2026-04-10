//! Activity panel rendering (right side) - top-level panel layout,
//! tools section, agents summary

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::RightPaneTab;
use crate::app::App;
use crate::pipeline::UnifiedTrustData;
use crate::ui::icons;
use crate::ui::widgets::TrustPanel;

use super::activity_tools::inline_agent_icon_color;

impl App {
    /// Render the activity panel (right side) with tools, agents, and status
    pub(super) fn render_activity_panel(&mut self, f: &mut Frame, area: Rect) {
        let bg_color = Color::Reset;
        f.render_widget(Clear, area);

        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(inner_area);
        let summary_area = content_chunks[0];
        let detail_area = content_chunks[1];
        self.layout.last_activity_rect = Some(summary_area);
        self.layout.last_agent_detail_rect = Some(detail_area);

        let mut summary_lines: Vec<Line<'_>> = Vec::new();
        self.layout.activity_agent_y_positions.clear();
        let mut current_line = 0usize;

        summary_lines.push(Line::from(vec![
            Span::styled(
                "Console",
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  (Ctrl+Up/Down select)",
                Style::default().fg(self.ui.theme.ui.text_dim),
            ),
        ]));
        current_line += 1;
        summary_lines.push(Line::raw(""));
        current_line += 1;

        // Main Session Entry
        current_line = self.render_activity_main_session(&mut summary_lines, current_line);

        // Coordination Pipeline
        current_line = self.render_activity_coordination(&mut summary_lines, current_line);

        // Spawned Agents
        current_line = self.render_activity_agents(&mut summary_lines, current_line);

        summary_lines.push(Line::raw(""));

        // Git Changes
        self.render_activity_git_changes(&mut summary_lines, current_line, summary_area);

        self.ui.activity_content_lines = summary_lines.len();
        let summary_para = Paragraph::new(Text::from(summary_lines))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Rgb(40, 40, 50)))
                    .style(Style::default().bg(bg_color)),
            )
            .wrap(Wrap { trim: true })
            .scroll((self.ui.activity_scroll_offset as u16, 0));
        f.render_widget(summary_para, summary_area);

        let detail_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(detail_area);
        self.layout.last_tab_bar_rect = Some(detail_chunks[0]);
        self.render_right_pane_tabs(f, detail_chunks[0], bg_color);
        match self.selected_right_pane_tab {
            RightPaneTab::Agent => self.render_selected_agent_detail(f, detail_chunks[1], bg_color),
            RightPaneTab::Diff => self.render_compact_diff_preview(f, detail_chunks[1], bg_color),
            RightPaneTab::Batch => self.render_batch_detail(f, detail_chunks[1], bg_color),
            RightPaneTab::Trust => self.render_trust_panel(f, detail_chunks[1], bg_color),
        }
    }

    fn render_trust_panel(&self, f: &mut Frame, area: Rect, _bg_color: Color) {
        let task_record = self.selected_task_record();

        let trust_data = task_record
            .as_ref()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t.metadata).ok())
            .map(|v| UnifiedTrustData::from_metadata(&v));

        let panel = TrustPanel::new(
            trust_data.as_ref().and_then(|t| t.merge_readiness.as_ref()),
            trust_data.as_ref().and_then(|t| t.review_summary.as_ref()),
            trust_data
                .as_ref()
                .and_then(|t| t.validation_summary.as_ref()),
            trust_data.as_ref().and_then(|t| t.qa_status.as_ref()),
        );
        panel.render(f, area);
    }

    fn render_activity_main_session(
        &mut self,
        summary_lines: &mut Vec<Line<'_>>,
        mut current_line: usize,
    ) -> usize {
        let is_main_selected = self.agents.selected_inline_agent == Some(usize::MAX);
        let session_style = if is_main_selected {
            Style::default()
                .fg(self.ui.theme.brand)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.ui.theme.ui.text)
        };
        self.layout.activity_agent_y_positions.push(current_line);
        summary_lines.push(Line::from(vec![
            Span::styled(
                if is_main_selected { "-> " } else { "  " },
                Style::default().fg(self.ui.theme.brand),
            ),
            Span::styled(
                format!("{} ", icons::status::info()),
                Style::default().fg(self.ui.theme.brand),
            ),
            Span::styled("Main Session", session_style),
        ]));
        current_line += 1;
        summary_lines.push(Line::raw(""));
        current_line += 1;
        current_line
    }

    fn render_activity_coordination(
        &mut self,
        summary_lines: &mut Vec<Line<'_>>,
        mut current_line: usize,
    ) -> usize {
        if !self.agents.parallel_batches.is_empty() {
            summary_lines.push(Line::from(vec![Span::styled(
                "Coordination",
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            current_line += 1;

            for batch in self.agents.parallel_batches.values() {
                summary_lines.push(Line::from(vec![
                    Span::styled(
                        format!(" #{}", &batch.id[..batch.id.len().min(8)]),
                        Style::default()
                            .fg(self.ui.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" coordination active"),
                ]));
                current_line += 1;
            }
            summary_lines.push(Line::raw(""));
            current_line += 1;
        }
        current_line
    }

    fn render_activity_agents(
        &mut self,
        summary_lines: &mut Vec<Line<'_>>,
        mut current_line: usize,
    ) -> usize {
        if self.agents.inline_agents.is_empty() {
            return current_line;
        }

        // Use the summary area width for truncation — fall back to 40 if unknown
        let max_task_chars = self
            .layout
            .last_activity_rect
            .map(|r| r.width.saturating_sub(14) as usize) // "- > icon ... [Xm Ys]" overhead
            .unwrap_or(28)
            .max(12);

        summary_lines.push(Line::from(vec![Span::styled(
            format!("Agents ({})", self.agents.inline_agents.len()),
            Style::default()
                .fg(self.ui.theme.brand_secondary)
                .add_modifier(Modifier::BOLD),
        )]));
        current_line += 1;

        for (idx, agent) in self.agents.inline_agents.iter().enumerate() {
            let (icon, color) = inline_agent_icon_color(agent.status, &self.ui.theme);
            let selected = self.agents.selected_inline_agent == Some(idx);
            let task_style = if selected {
                Style::default()
                    .fg(Color::Rgb(80, 255, 150))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.ui.theme.ui.text)
            };

            let short_task = truncate_to_words(&agent.task, max_task_chars);

            self.layout.activity_agent_y_positions.push(current_line);
            summary_lines.push(Line::from(vec![
                Span::styled(
                    if selected { "-> " } else { "  " },
                    Style::default().fg(self.ui.theme.brand),
                ),
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(short_task, task_style),
                Span::styled(
                    format!(" [{}]", agent.elapsed()),
                    Style::default().fg(self.ui.theme.ui.text_dim),
                ),
            ]));
            current_line += 1;
        }
        current_line
    }
}

/// Truncate a task description to 2-3 words that fit within `max_chars`.
///
/// Strategy: take whole words until we'd exceed the budget, then append "..".
/// Falls back to hard-char truncation if the first word itself is too long.
fn truncate_to_words(task: &str, max_chars: usize) -> String {
    if task.len() <= max_chars {
        return task.to_string();
    }

    let mut used = 0;
    let mut words: Vec<&str> = Vec::new();
    for word in task.split_whitespace() {
        // +1 for the space between words (except before the first)
        let needed = word.len() + if words.is_empty() { 0 } else { 1 };
        if used + needed > max_chars.saturating_sub(2) {
            // Reserve 2 chars for ".."
            break;
        }
        words.push(word);
        used += needed;
    }

    if words.is_empty() {
        // First word is too long — hard-truncate it
        format!("{}..", &task[..max_chars.saturating_sub(2).max(1)])
    } else {
        format!("{}..", words.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_words_short_enough() {
        assert_eq!(truncate_to_words("Fix bug", 20), "Fix bug");
    }

    #[test]
    fn test_truncate_to_words_truncates() {
        assert_eq!(
            truncate_to_words("Fix the authentication bug in the login module", 18),
            "Fix the.."
        );
    }

    #[test]
    fn test_truncate_to_words_single_long_word() {
        assert_eq!(
            truncate_to_words("superlongwordthatdoesnotfit", 10),
            "superlon.."
        );
    }

    #[test]
    fn test_truncate_to_words_exact_fit() {
        assert_eq!(truncate_to_words("Fix auth", 8), "Fix auth");
    }

    #[test]
    fn test_truncate_to_words_two_words_then_dot() {
        assert_eq!(
            truncate_to_words("Refactor the entire authentication flow", 16),
            "Refactor the.."
        );
    }
}
