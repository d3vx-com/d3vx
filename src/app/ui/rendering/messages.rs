//! Chat message rendering

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::ipc::MessageRole;
use crate::ui::symbols::{AI_INDICATOR, SHELL_INDICATOR, USER_INDICATOR};
use crate::utils::format::format_elapsed;

use crate::app::ui::helpers::{braille_frame, MUTED_WHITE};

impl App {
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

            // Tool calls are shown in the activity panel, not in chat
            lines.push(Line::raw("")); // Spacing
        }

        // Add thinking indicator inline in chat
        if self.session.thinking.is_thinking || self.has_background_activity() {
            let elapsed = self
                .session
                .thinking_start
                .map(|s| s.elapsed().as_secs())
                .unwrap_or(0);

            let subagent_count = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.subagents.list())
                    .iter()
                    .filter(|a| a.status == crate::agent::SubAgentStatus::Running)
                    .count()
            });

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
