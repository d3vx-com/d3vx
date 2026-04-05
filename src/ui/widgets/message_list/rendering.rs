use super::types::*;
use crate::ipc::{Message, MessageRole};
use crate::ui::symbols::{AI_INDICATOR, SHELL_INDICATOR, STATUS, USER_INDICATOR};
use crate::ui::theme::Theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};

impl<'a> MessageList<'a> {
    /// Create a new message list
    pub fn new(messages: &'a [Message]) -> Self {
        Self {
            messages,
            verbose: false,
            max_visible: 100,
            scroll_offset: 0,
            theme: Theme::dark(),
            truncate: TruncateConfig::default(),
        }
    }

    /// Set verbose mode
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set maximum visible messages
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// Set scroll offset
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Get visible messages based on scroll offset
    fn get_visible_messages(&self) -> &[Message] {
        let effective_max = self.max_visible.min(self.messages.len());
        let max_scroll = self.messages.len().saturating_sub(effective_max);
        let safe_offset = self.scroll_offset.min(max_scroll);

        let start = self
            .messages
            .len()
            .saturating_sub(effective_max + safe_offset);
        let end = self.messages.len().saturating_sub(safe_offset);

        &self.messages[start..end]
    }

    /// Build the message list into lines
    pub fn build_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let visible = self.get_visible_messages();

        for message in visible {
            self.build_message_lines(message, &mut lines);
            lines.push(Line::raw(""));
        }

        lines
    }

    /// Build lines for a single message
    fn build_message_lines(&self, msg: &Message, lines: &mut Vec<Line<'static>>) {
        match msg.role {
            MessageRole::Shell => self.build_shell_message_lines(msg, lines),
            _ => self.build_standard_message_lines(msg, lines),
        }
    }

    /// Build lines for a shell message
    fn build_shell_message_lines(&self, msg: &Message, lines: &mut Vec<Line<'static>>) {
        let shell_cmd = msg.shell_cmd.clone().unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(
                SHELL_INDICATOR,
                Style::default()
                    .fg(self.theme.role.shell)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(shell_cmd, Style::default().fg(self.theme.role.shell)),
        ]));

        for line in msg.content.lines() {
            lines.push(Line::from(format!("  {}", line)));
        }

        if let Some(exit_code) = msg.exit_code {
            let success = exit_code == 0;
            let color = if success {
                self.theme.state.success
            } else {
                self.theme.state.error
            };
            let icon = if success {
                STATUS.success
            } else {
                STATUS.error
            };

            lines.push(Line::from(vec![Span::styled(
                format!("  {} exit {}", icon, exit_code),
                Style::default().fg(color),
            )]));
        }
    }

    /// Build lines for a standard message (user/assistant/system)
    fn build_standard_message_lines(&self, msg: &Message, lines: &mut Vec<Line<'static>>) {
        let is_user = msg.role == MessageRole::User;
        let is_error = msg.is_error;

        let (role_icon, role_color, role_name) = if is_user {
            (USER_INDICATOR, self.theme.role.user, "You")
        } else if is_error {
            (AI_INDICATOR, self.theme.state.error, "Error")
        } else {
            (AI_INDICATOR, self.theme.role.assistant, "d3vx")
        };

        lines.push(Line::from(vec![
            Span::styled(
                role_icon,
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                role_name,
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ),
        ]));

        for line in msg.content.lines() {
            lines.push(Line::raw(line.to_string()));
        }

        if !msg.tool_calls.is_empty() {
            self.build_tool_call_lines(&msg.tool_calls, lines);
        }
    }
}

impl<'a> Widget for MessageList<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let lines = self.build_lines();
        let paragraph = Paragraph::new(Text::from(lines));
        paragraph.render(area, buf);
    }
}
