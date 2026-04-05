//! Agent detail inspector rendering

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::InlineAgentStatus;
use crate::app::App;
use crate::ui::symbols::AI_INDICATOR;

use crate::app::ui::helpers::{braille_frame, MUTED_WHITE};

impl App {
    pub(super) fn render_selected_agent_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        bg_color: Color,
    ) {
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        f.render_widget(Block::default().style(Style::default().bg(bg_color)), area);

        // Handle Main Session inspection
        if self.agents.selected_inline_agent == Some(usize::MAX) {
            self.render_main_session_detail(f, inner, bg_color);
            return;
        }

        let Some(index) = self.agents.selected_inline_agent else {
            self.ui.selected_agent_output_lines = 0;
            let empty = Paragraph::new(
                "Select an activity or agent from the console above to inspect details.",
            )
            .style(Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color))
            .wrap(Wrap { trim: true });
            f.render_widget(empty, inner);
            return;
        };
        if index >= self.agents.inline_agents.len() {
            return;
        }

        let agent = &self.agents.inline_agents[index];
        let mut detail_lines: Vec<Line<'_>> = Vec::new();

        // Header: Braille Spinner + Task Title
        let (status_icon, status_color) = agent_status_style(agent.status, self);

        detail_lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(status_color),
            ),
            Span::styled(
                agent.task.clone(),
                Style::default()
                    .fg(self.ui.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Status Line
        let status_text = match agent.status {
            InlineAgentStatus::Running => "running",
            InlineAgentStatus::Completed => "completed",
            InlineAgentStatus::Ended => "ended",
            InlineAgentStatus::Failed => "failed",
            InlineAgentStatus::Cancelled => "cancelled",
        };

        detail_lines.push(Line::from(vec![
            Span::styled(
                format!("{}  ", status_text),
                Style::default().fg(status_color),
            ),
            Span::styled(
                format!("{} calls", agent.tool_count),
                Style::default().fg(self.ui.theme.ui.text_dim),
            ),
            Span::styled(" \u{00b7} ", Style::default().fg(self.ui.theme.ui.border)),
            Span::styled(
                agent.elapsed(),
                Style::default().fg(self.ui.theme.ui.text_dim),
            ),
        ]));
        detail_lines.push(Line::raw(""));

        // Tool Summary
        if !agent.tools_used.is_empty() {
            let recent_tools: Vec<&str> = agent
                .tools_used
                .iter()
                .rev()
                .take(3)
                .map(|s| s.as_str())
                .collect();
            let tools_str = recent_tools
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(", ");

            detail_lines.push(Line::from(vec![
                Span::styled("tools: ", Style::default().fg(self.ui.theme.ui.text_dim)),
                Span::styled(tools_str, Style::default().fg(self.ui.theme.ui.text_dim)),
            ]));
        }

        // Current Action (if running)
        if agent.status == InlineAgentStatus::Running {
            if let Some(action) = &agent.current_action {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", braille_frame()),
                        Style::default().fg(MUTED_WHITE),
                    ),
                    Span::styled(action.clone(), Style::default().fg(MUTED_WHITE)),
                ]));
            }
        }
        detail_lines.push(Line::raw(""));

        // Clean Text Transcript (No tool noise)
        let text_messages: Vec<_> = agent
            .messages
            .iter()
            .filter(|m| m.line_type == crate::app::state::AgentLineType::Text)
            .collect();

        if !text_messages.is_empty() {
            detail_lines.push(Line::from(vec![Span::styled(
                "Transcript",
                Style::default()
                    .fg(self.ui.theme.ui.text_dim)
                    .add_modifier(Modifier::BOLD),
            )]));

            for msg in text_messages.iter().rev().take(5).rev() {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", AI_INDICATOR),
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                    Span::styled(
                        "Assistant",
                        Style::default().fg(self.ui.theme.ui.text_muted),
                    ),
                ]));

                let md_widget = crate::ui::widgets::MarkdownText::new(&msg.content)
                    .theme(self.ui.theme.clone());
                for line in md_widget.render() {
                    detail_lines.push(Line::from(
                        line.spans
                            .into_iter()
                            .map(|s| Span::styled(s.content, s.style))
                            .collect::<Vec<_>>(),
                    ));
                }
                detail_lines.push(Line::raw(""));
            }
        }

        self.ui.selected_agent_output_lines = detail_lines.len();
        let max_scroll = self
            .ui
            .selected_agent_output_lines
            .saturating_sub(inner.height as usize);
        if self.ui.selected_agent_output_scroll > max_scroll {
            self.ui.selected_agent_output_scroll = max_scroll;
        }

        let paragraph = Paragraph::new(Text::from(detail_lines))
            .style(Style::default().bg(bg_color))
            .wrap(Wrap { trim: true })
            .scroll((self.ui.selected_agent_output_scroll as u16, 0));
        f.render_widget(paragraph, inner);
    }
}

/// Get the status icon and color for an inline agent
fn agent_status_style(status: InlineAgentStatus, app: &App) -> (String, Color) {
    match status {
        InlineAgentStatus::Running => (braille_frame().to_string(), MUTED_WHITE),
        InlineAgentStatus::Completed => (
            crate::ui::symbols::STATUS.success.to_string(),
            Color::Rgb(80, 200, 120),
        ),
        InlineAgentStatus::Ended => ("\u{2500}".to_string(), Color::Rgb(100, 180, 120)),
        InlineAgentStatus::Failed => (
            crate::ui::symbols::STATUS.error.to_string(),
            Color::Rgb(220, 100, 100),
        ),
        InlineAgentStatus::Cancelled => ("\u{2500}".to_string(), app.ui.theme.ui.text_dim),
    }
}
