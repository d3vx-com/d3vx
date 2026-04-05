//! Agent View Widget
//!
//! Scrollable view of raw agent events from .jsonl session logs.

use crate::agent::agent_loop::AgentEvent;
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct AgentView {
    events: Vec<AgentEvent>,
    theme: Theme,
    scroll: usize,
}

impl AgentView {
    pub fn new(events: Vec<AgentEvent>, theme: Theme) -> Self {
        Self {
            events,
            theme,
            scroll: 0,
        }
    }

    pub fn scroll(mut self, offset: usize) -> Self {
        self.scroll = offset;
        self
    }
}

impl Widget for AgentView {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let block = Block::default()
            .title(" AGENT_LOG_STREAM ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.ui.border));

        let mut lines = Vec::new();

        for event in &self.events {
            match event {
                AgentEvent::Text { text } => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "助理: ",
                            Style::default()
                                .fg(self.theme.brand)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(text),
                    ]));
                }
                AgentEvent::Thinking { text } => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "思考: ",
                            Style::default()
                                .fg(self.theme.ui.text_dim)
                                .add_modifier(Modifier::ITALIC),
                        ),
                        Span::styled(text, Style::default().fg(self.theme.ui.text_dim)),
                    ]));
                }
                AgentEvent::ToolStart { id: _, name } => {
                    lines.push(Line::from(vec![
                        Span::styled("工具: ", Style::default().fg(self.theme.brand_secondary)),
                        Span::styled(
                            format!("Calling {}...", name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                AgentEvent::ToolEnd {
                    id: _,
                    name,
                    result: _,
                    is_error,
                    elapsed_ms,
                } => {
                    let color = if *is_error {
                        self.theme.state.error
                    } else {
                        self.theme.state.success
                    };
                    lines.push(Line::from(vec![
                        Span::styled("工具: ", Style::default().fg(self.theme.brand_secondary)),
                        Span::styled(
                            format!("{} finished ({}ms)", name, elapsed_ms),
                            Style::default().fg(color),
                        ),
                    ]));
                }
                _ => {}
            }
            lines.push(Line::raw(""));
        }

        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((self.scroll as u16, 0))
            .render(area, buf);
    }
}
