//! Chat message rendering

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::ipc::{MessageRole, ToolCall, ToolStatus};
use crate::ui::symbols::{AI_INDICATOR, SHELL_INDICATOR, STATUS, USER_INDICATOR};
use crate::utils::format::format_elapsed;

use crate::app::ui::helpers::{braille_frame, MUTED_WHITE};

impl App {
    /// Render a tool call inline (Claude Code style)
    fn render_inline_tool(&self, lines: &mut Vec<Line<'_>>, tool: &ToolCall) {
        let theme = &self.ui.theme;

        // Tool header line with status indicator
        let status_icon = match tool.status {
            ToolStatus::Pending => "○",
            ToolStatus::Running => "◐",
            ToolStatus::Completed => STATUS.success,
            ToolStatus::Error => STATUS.error,
            ToolStatus::WaitingApproval => "○",
        };

        let status_color = match tool.status {
            ToolStatus::Pending => theme.ui.text_dim,
            ToolStatus::Running => theme.brand_secondary,
            ToolStatus::Completed => theme.state.success,
            ToolStatus::Error => theme.state.error,
            ToolStatus::WaitingApproval => theme.ui.text_dim,
        };

        // Truncate long tool names for display
        let tool_name = if tool.name.len() > 50 {
            format!("{}...", &tool.name[..47])
        } else {
            tool.name.clone()
        };

        // Build header with elapsed time if available
        let header_text = if let Some(elapsed) = tool.elapsed {
            let elapsed_str = if elapsed < 1000 {
                format!("{}ms", elapsed)
            } else {
                format!("{:.1}s", elapsed as f64 / 1000.0)
            };
            format!("{} {} ({})", status_icon, tool_name, elapsed_str)
        } else {
            format!("{} {}", status_icon, tool_name)
        };

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("◇", Style::default().fg(theme.ui.text_dim)),
            Span::styled(" ", Style::default()),
            Span::styled(
                header_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Show tool input (truncated)
        if !tool.input.is_null() {
            let input_str = tool.input.to_string();
            let truncated = if input_str.len() > 100 {
                format!("{}...", &input_str[..97])
            } else {
                input_str
            };
            lines.push(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    truncated,
                    Style::default()
                        .fg(theme.ui.text_dim)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        // Show tool output if available (truncated)
        if let Some(ref output) = tool.output {
            let truncated = if output.len() > 150 {
                format!("{}...", &output[..147])
            } else {
                output.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    truncated,
                    Style::default().fg(match tool.status {
                        ToolStatus::Error => theme.state.error,
                        ToolStatus::WaitingApproval => theme.ui.text_dim,
                        _ => theme.ui.text,
                    }),
                ),
            ]));
        }
    }

    /// Render tool calls in collapsed mode.
    ///
    /// If 3 or fewer tools: show all.
    /// If more than 3: show summary line + last 3 running/pending + any errors.
    fn render_collapsed_tools(&self, lines: &mut Vec<Line<'_>>, tools: &[ToolCall]) {
        let running: Vec<&ToolCall> = tools
            .iter()
            .filter(|t| matches!(t.status, ToolStatus::Running | ToolStatus::Pending))
            .collect();
        let completed = tools
            .iter()
            .filter(|t| t.status == ToolStatus::Completed)
            .count();
        let errors = tools
            .iter()
            .filter(|t| t.status == ToolStatus::Error)
            .count();
        let total = tools.len();

        // If 3 or fewer, just show all
        if total <= 3 {
            for tool in tools {
                self.render_inline_tool(lines, tool);
            }
            return;
        }

        // Summary line: "◇ 8 tools called (5 done, 1 failed)"
        let theme = &self.ui.theme;
        let mut summary_parts = vec![format!("{} tools called", total)];
        if completed > 0 {
            summary_parts.push(format!("{} done", completed));
        }
        if errors > 0 {
            summary_parts.push(format!("{} failed", errors));
        }
        let summary_text = if summary_parts.len() > 1 {
            format!("{} ({})", summary_parts[0], summary_parts[1..].join(", "))
        } else {
            summary_parts[0].clone()
        };

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("◇", Style::default().fg(theme.ui.text_dim)),
            Span::styled(" ", Style::default()),
            Span::styled(summary_text, Style::default().fg(theme.ui.text_dim)),
            Span::styled(
                " — Ctrl+O to expand",
                Style::default()
                    .fg(theme.ui.text_dim)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));

        // Show last 3 running/pending tools
        for tool in running.iter().rev().take(3).rev() {
            self.render_inline_tool(lines, tool);
        }

        // Always show errors (last 2)
        let error_tools: Vec<&ToolCall> = tools
            .iter()
            .filter(|t| t.status == ToolStatus::Error)
            .collect();
        for tool in error_tools.iter().rev().take(2).rev() {
            self.render_inline_tool(lines, tool);
        }
    }

    /// Render messages
    pub fn render_messages(&mut self, area: Rect) -> Paragraph<'_> {
        let mut lines: Vec<Line<'_>> = Vec::new();

        for msg in &self.session.messages {
            // Role indicator
            let role_icon = match msg.role {
                MessageRole::User => USER_INDICATOR,
                MessageRole::Assistant => AI_INDICATOR,
                MessageRole::System => AI_INDICATOR,
                MessageRole::Shell => SHELL_INDICATOR,
            };

            // Role header
            let role_name = match msg.role {
                MessageRole::User => "You",
                MessageRole::Assistant => "d3vx",
                MessageRole::System => "System",
                MessageRole::Shell => "Shell",
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", role_icon),
                    Style::default().fg(self.ui.theme.ui.text_dim),
                ),
                Span::styled(
                    role_name,
                    Style::default()
                        .fg(self.ui.theme.ui.text_muted)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            // Markdown Rendered Content
            let md_widget =
                crate::ui::widgets::MarkdownText::new(&msg.content).theme(self.ui.theme.clone());
            for line in md_widget.render() {
                lines.push(line);
            }

            // Inline tool calls — collapsed or expanded
            if !msg.tool_calls.is_empty() {
                if self.tools.chat_tools_expanded {
                    for tool in &msg.tool_calls {
                        self.render_inline_tool(&mut lines, tool);
                    }
                } else {
                    self.render_collapsed_tools(&mut lines, &msg.tool_calls);
                }
            }

            lines.push(Line::raw("")); // Spacing
        }

        // Add thinking indicator inline in chat
        if self.session.thinking.is_thinking || self.has_background_activity() {
            let elapsed = self
                .session
                .thinking_start
                .map(|s| s.elapsed().as_secs())
                .unwrap_or(0);

            // Use cached subagent count (updated in update loop)
            let subagent_count = self.cached_subagent_count;

            // Build thinking status text
            let mut status_parts = Vec::new();

            if self.session.thinking.is_thinking {
                let phase = format!("{:?}", self.session.thinking.phase);
                status_parts.push(phase);
            }

            if subagent_count > 0 {
                status_parts.push(format!("{} agent(s)", subagent_count));
            }

            if !self.background_active_tasks.is_empty() {
                status_parts.push(format!("{} bg tasks", self.background_active_tasks.len()));
            }

            let status_text = if status_parts.is_empty() {
                "Working...".to_string()
            } else {
                status_parts.join(" | ")
            };

            // Animated thinking indicator
            let thinking_char = braille_frame();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", AI_INDICATOR),
                    Style::default().fg(self.ui.theme.ui.text_dim),
                ),
                Span::styled(
                    "d3vx",
                    Style::default()
                        .fg(self.ui.theme.ui.text_muted)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ]));

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", thinking_char),
                    Style::default().fg(MUTED_WHITE),
                ),
                Span::styled(status_text, Style::default().fg(self.ui.theme.ui.text)),
                if elapsed > 0 {
                    Span::styled(
                        format!(" ({})", format_elapsed(elapsed)),
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    )
                } else {
                    Span::raw("")
                },
            ]));

            // Show thinking text if available
            if !self.session.thinking.text.is_empty() {
                let thinking_preview = if self.session.thinking.text.len() > 80 {
                    format!("{}..", &self.session.thinking.text[..78])
                } else {
                    self.session.thinking.text.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default().fg(self.ui.theme.ui.text_dim)),
                    Span::styled(
                        thinking_preview,
                        Style::default()
                            .fg(self.ui.theme.ui.text_dim)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }

            lines.push(Line::raw("")); // Spacing
        }

        let mut total_lines: usize = 0;
        for line in &lines {
            let w = line.width() as usize;
            if w == 0 {
                total_lines += 1;
            } else {
                total_lines += (w.saturating_sub(1) / area.width.max(1) as usize) + 1;
            }
        }

        let max_scroll = total_lines.saturating_sub(area.height as usize);
        self.ui.max_scroll.set(max_scroll);
        let safe_scroll_offset = self.ui.scroll_offset.min(max_scroll);
        let ratatui_scroll = max_scroll.saturating_sub(safe_scroll_offset);

        // Store total lines for click detection
        self.layout.chat_total_lines = total_lines;

        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((ratatui_scroll as u16, 0))
    }
}
