//! Inline Agent Card Widget
//!
//! Renders collapsible cards for inline spawned agents (running in-process without worktrees).

use crate::app::state::{AgentLineType, AgentMessageLine, InlineAgentInfo, InlineAgentStatus};
use crate::app::ui::helpers::{braille_frame, MUTED_WHITE};
use crate::ui::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

pub struct InlineAgentCard<'a> {
    agents: &'a [InlineAgentInfo],
    theme: &'a Theme,
    selected_index: Option<usize>,
    width: u16,
}

impl<'a> InlineAgentCard<'a> {
    pub fn new(agents: &'a [InlineAgentInfo], theme: &'a Theme, width: u16) -> Self {
        Self {
            agents,
            theme,
            selected_index: None,
            width,
        }
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }

    fn render_collapsed(
        &self,
        agent: &InlineAgentInfo,
        index: usize,
        y: &mut u16,
        buf: &mut Buffer,
        area: Rect,
    ) {
        if *y >= area.y + area.height {
            return;
        }

        let is_selected = self.selected_index == Some(index);

        // Braille spinner for running/thinking, static icons for terminal states
        let (icon, icon_color) = match agent.status {
            InlineAgentStatus::Running => (braille_frame(), MUTED_WHITE),
            InlineAgentStatus::Completed => ("✓", self.theme.state.success),
            InlineAgentStatus::Ended => ("─", Color::Rgb(100, 180, 120)),
            InlineAgentStatus::Failed => ("✗", self.theme.state.error),
            InlineAgentStatus::Cancelled => ("─", self.theme.ui.text_dim),
        };

        // Selected agents get green highlight
        let task_style = if is_selected {
            Style::default()
                .fg(self.theme.state.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.ui.text)
        };

        let expand = if agent.expanded { "[-]" } else { "[+]" };

        // Show tool indicator if tools are visible
        let tools_indicator = if agent.show_tools {
            Span::styled(" T", Style::default().fg(self.theme.state.info))
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(expand, Style::default().fg(self.theme.ui.text_dim)),
            tools_indicator,
            Span::raw(" "),
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::raw(" "),
            Span::styled(&agent.task, task_style),
            Span::raw(" "),
            Span::styled(
                agent.progress_summary(),
                Style::default().fg(self.theme.ui.text_dim),
            ),
        ]);

        line.render(Rect::new(area.x, *y, area.width, 1), buf);
        *y += 1;
    }

    fn render_expanded(
        &self,
        agent: &InlineAgentInfo,
        index: usize,
        y: &mut u16,
        buf: &mut Buffer,
        area: Rect,
    ) {
        if *y >= area.y + area.height {
            return;
        }

        let is_selected = self.selected_index == Some(index);

        let (status_icon, status_color) = match agent.status {
            InlineAgentStatus::Running => (braille_frame(), MUTED_WHITE),
            InlineAgentStatus::Completed => ("✓", self.theme.state.success),
            InlineAgentStatus::Ended => ("─", Color::Rgb(100, 180, 120)),
            InlineAgentStatus::Failed => ("✗", self.theme.state.error),
            InlineAgentStatus::Cancelled => ("─", self.theme.ui.text_dim),
        };

        let status_text = match agent.status {
            InlineAgentStatus::Running => "Running",
            InlineAgentStatus::Completed => "Completed",
            InlineAgentStatus::Ended => "Ended",
            InlineAgentStatus::Failed => "Failed",
            InlineAgentStatus::Cancelled => "Cancelled",
        };

        // Header line - selected agents get green highlight
        let task_style = if is_selected {
            Style::default()
                .fg(self.theme.state.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.theme.ui.text)
                .add_modifier(Modifier::BOLD)
        };

        let header = Line::from(vec![
            Span::styled("  [-] ", Style::default().fg(self.theme.ui.text_dim)),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(&agent.task, task_style),
            Span::raw("  "),
            Span::styled(status_text, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(agent.elapsed(), Style::default().fg(self.theme.ui.text_dim)),
        ]);
        header.render(Rect::new(area.x, *y, area.width, 1), buf);
        *y += 1;

        if *y >= area.y + area.height {
            return;
        }

        // ── Tool summary line: "8 calls · read_file, write_file, bash"
        if agent.tool_count > 0 && *y < area.y + area.height {
            // Show last 3 unique tool names
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

            let tool_line = Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{} calls", agent.tool_count),
                    Style::default().fg(self.theme.ui.text_dim),
                ),
                Span::styled(" · ", Style::default().fg(self.theme.ui.border)),
                Span::styled(tools_str, Style::default().fg(self.theme.ui.text_dim)),
            ]);
            tool_line.render(Rect::new(area.x, *y, area.width, 1), buf);
            *y += 1;
        }

        // ── Current action (if running)
        if agent.status == InlineAgentStatus::Running {
            if let Some(ref action) = agent.current_action {
                if *y < area.y + area.height {
                    let action_line = Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("{} ", braille_frame()),
                            Style::default().fg(MUTED_WHITE),
                        ),
                        Span::styled(action, Style::default().fg(MUTED_WHITE)),
                    ]);
                    action_line.render(Rect::new(area.x, *y, area.width, 1), buf);
                    *y += 1;
                }
            }
        }

        // ── Text output only (no tool calls/outputs/thinking noise) - unless show_tools is enabled
        let messages_to_show: Vec<&AgentMessageLine> = if agent.show_tools {
            // Show all message types including tool calls and outputs
            agent
                .messages
                .iter()
                .filter(|m| {
                    matches!(
                        m.line_type,
                        AgentLineType::Text | AgentLineType::ToolCall | AgentLineType::ToolOutput
                    )
                })
                .collect()
        } else {
            // Only text messages
            agent
                .messages
                .iter()
                .filter(|m| m.line_type == AgentLineType::Text)
                .collect()
        };

        if !messages_to_show.is_empty() && *y < area.y + area.height {
            // Blank separator
            *y += 1;

            // Show last few messages that fit
            let remaining = (area.y + area.height).saturating_sub(*y) as usize;
            let max_lines = remaining.min(if agent.show_tools { 12 } else { 6 });
            let mut lines_used = 0;

            for msg in messages_to_show
                .iter()
                .rev()
                .take(if agent.show_tools { 6 } else { 2 })
                .rev()
            {
                if lines_used >= max_lines {
                    break;
                }

                // Add label for tool calls/outputs
                let prefix = match msg.line_type {
                    AgentLineType::ToolCall => "    [Tool] ",
                    AgentLineType::ToolOutput => "    [Out]  ",
                    _ => "    ",
                };
                let prefix_style = match msg.line_type {
                    AgentLineType::ToolCall => Style::default().fg(self.theme.state.info),
                    AgentLineType::ToolOutput => Style::default().fg(self.theme.ui.text_dim),
                    _ => Style::default(),
                };

                let content_lines: Vec<&str> = msg.content.lines().collect();
                for line in content_lines.iter().take(max_lines - lines_used) {
                    if *y < area.y + area.height {
                        let display = if line.len() > (self.width as usize).saturating_sub(12) {
                            let end = (self.width as usize).saturating_sub(15);
                            format!("{}...", &line[..end.min(line.len())])
                        } else {
                            line.to_string()
                        };
                        let content_line = Line::from(vec![
                            Span::styled(prefix, prefix_style),
                            Span::styled(display, Style::default().fg(self.theme.ui.text)),
                        ]);
                        content_line.render(Rect::new(area.x, *y, area.width, 1), buf);
                        *y += 1;
                        lines_used += 1;
                    }
                }
            }
        }

        // Actions hint
        if agent.status == InlineAgentStatus::Running && *y < area.y + area.height {
            let tools_label = if agent.show_tools {
                "[Hide Tools]"
            } else {
                "[Tools]"
            };
            let actions = Line::from(vec![
                Span::raw("  "),
                Span::styled("[Stop]", Style::default().fg(self.theme.state.error)),
                Span::raw(" "),
                Span::styled(tools_label, Style::default().fg(self.theme.state.info)),
                Span::raw(" "),
                Span::styled("[Collapse]", Style::default().fg(self.theme.ui.text_dim)),
            ]);
            actions.render(Rect::new(area.x, *y, area.width, 1), buf);
            *y += 1;
        } else if agent.status != InlineAgentStatus::Running && *y < area.y + area.height {
            // Show tools toggle for completed agents too
            let tools_label = if agent.show_tools {
                "[Hide Tools]"
            } else {
                "[Tools]"
            };
            let actions = Line::from(vec![
                Span::raw("  "),
                Span::styled(tools_label, Style::default().fg(self.theme.state.info)),
                Span::raw(" "),
                Span::styled("[Collapse]", Style::default().fg(self.theme.ui.text_dim)),
            ]);
            actions.render(Rect::new(area.x, *y, area.width, 1), buf);
            *y += 1;
        }
    }

    fn render_separator(&self, y: &mut u16, buf: &mut Buffer, area: Rect) {
        if *y >= area.y + area.height {
            return;
        }
        let sep = Line::from(vec![Span::raw("  "); area.width as usize]);
        sep.render(Rect::new(area.x, *y, area.width, 1), buf);
        *y += 1;
    }
}

impl<'a> Widget for InlineAgentCard<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.agents.is_empty() {
            return;
        }

        let mut y = area.y;

        // Header with spinner if any agents are running
        let any_running = self
            .agents
            .iter()
            .any(|a| a.status == InlineAgentStatus::Running);

        let header_icon = if any_running {
            Span::styled(
                format!("{} ", braille_frame()),
                Style::default().fg(MUTED_WHITE),
            )
        } else {
            Span::styled("✓ ", Style::default().fg(self.theme.state.success))
        };

        let header = Line::from(vec![
            Span::raw("  "),
            header_icon,
            Span::styled(
                "Parallel Agents ",
                Style::default()
                    .fg(self.theme.ui.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", self.agents.len()),
                Style::default().fg(self.theme.ui.text_dim),
            ),
        ]);
        header.render(Rect::new(area.x, y, area.width, 1), buf);
        y += 1;

        // Separator line
        let sep_style = Style::default().fg(self.theme.ui.border);
        let sep = Line::from(vec![
            Span::styled("  ", sep_style),
            Span::styled("─".repeat(area.width as usize - 4), sep_style),
        ]);
        sep.render(Rect::new(area.x, y, area.width, 1), buf);
        y += 1;

        // Render each agent
        for (i, agent) in self.agents.iter().enumerate() {
            if agent.expanded {
                self.render_expanded(agent, i, &mut y, buf, area);
                self.render_separator(&mut y, buf, area);
            } else {
                self.render_collapsed(agent, i, &mut y, buf, area);
            }
        }
    }
}

/// Renders a list of inline agent cards
pub struct InlineAgentList<'a> {
    agents: &'a [InlineAgentInfo],
    theme: &'a Theme,
    selected_index: Option<usize>,
}

impl<'a> InlineAgentList<'a> {
    pub fn new(agents: &'a [InlineAgentInfo], theme: &'a Theme) -> Self {
        Self {
            agents,
            theme,
            selected_index: None,
        }
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }
}

impl<'a> Widget for InlineAgentList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.agents.is_empty() {
            return;
        }

        let width = area.width;
        InlineAgentCard::new(self.agents, self.theme, width)
            .selected(self.selected_index)
            .render(area, buf);
    }
}
