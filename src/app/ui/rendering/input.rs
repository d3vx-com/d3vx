//! Input area and status bar rendering

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::FocusMode;
use crate::app::App;

impl App {
    /// Render input area — mode badge + prompt on row 1, hint strip on row 2
    pub fn render_input(&mut self, f: &mut Frame, area: Rect) {
        let inner_area = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 0,
        });

        // Split: input line (Min) + hint strip (Length 1)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner_area);

        let input_area = chunks[0];
        let hint_area = chunks[1];

        // ── Input line: [MODE] → prompt ──

        let mode = self.ui.focus_mode;
        let mode_label = mode.label();

        // Badge style: Chat is subtle, others use brand color
        let badge_style = if mode == FocusMode::Chat {
            Style::default()
                .bg(Color::Rgb(40, 40, 48))
                .fg(Color::Rgb(120, 120, 135))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(self.ui.theme.brand)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        };

        let mut prompt = self.ui.input_buffer.clone();

        // Blinking cursor
        let cursor_char = if (self.animation_frame / 4) % 2 == 0 {
            "\u{2503}"
        } else {
            " "
        };
        if self.ui.cursor_position <= prompt.len() {
            prompt.insert_str(self.ui.cursor_position, cursor_char);
        } else {
            prompt.push_str(cursor_char);
        }

        // Build input spans: [MODE] → prompt
        let mut spans = vec![
            Span::styled(format!(" {} ", mode_label), badge_style),
            Span::raw(" "),
            Span::styled("\u{2192} ", Style::default().fg(Color::Rgb(80, 200, 120))),
        ];

        let tokens: Vec<&str> = prompt.split_inclusive(char::is_whitespace).collect();

        if tokens.is_empty() || (tokens.len() == 1 && tokens[0] == cursor_char) {
            // Empty input — show subtle placeholder
            spans.push(Span::styled(
                "Type a message...",
                Style::default().fg(Color::Rgb(80, 80, 90)),
            ));
            if tokens.len() == 1 && tokens[0] == cursor_char {
                spans.insert(3, Span::raw(cursor_char));
            }
        } else {
            for token in tokens {
                let trimmed = token.trim();
                let style = if trimmed.starts_with('/')
                    || trimmed.starts_with('!')
                    || trimmed.starts_with("--")
                {
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else if trimmed.starts_with('@') && trimmed.len() > 1 {
                    Style::default()
                        .fg(self.ui.theme.brand_secondary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.ui.theme.ui.text)
                };

                spans.push(Span::styled(token.to_string(), style));
            }
        }

        // ── Hint strip ──
        //
        // Empty prompt → show the top 4 slash commands inline so a new
        // user learns the surface exists. Once they start typing the
        // hint reverts to the focus-mode description so it doesn't
        // compete with the prompt content.
        let hint_line = if self.ui.input_buffer.is_empty() {
            Line::from(vec![
                Span::styled(" \u{203A} ", Style::default().fg(Color::Rgb(90, 90, 105))),
                Span::styled(
                    "/board",
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(" kanban  ", Style::default().fg(Color::Rgb(70, 70, 82))),
                Span::styled(
                    "/list",
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(" tasks  ", Style::default().fg(Color::Rgb(70, 70, 82))),
                Span::styled(
                    "/dashboard",
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(" web  ", Style::default().fg(Color::Rgb(70, 70, 82))),
                Span::styled(
                    "/vex",
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(" bg  ", Style::default().fg(Color::Rgb(70, 70, 82))),
                Span::styled(
                    "?",
                    Style::default()
                        .fg(self.ui.theme.brand_secondary)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(" all", Style::default().fg(Color::Rgb(70, 70, 82))),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    format!(" {} ", mode.hint()),
                    Style::default().fg(Color::Rgb(70, 70, 82)),
                ),
                Span::styled("Ctrl+F cycle", Style::default().fg(Color::Rgb(45, 45, 55))),
            ])
        };

        // Render
        self.layout.last_mode_bar_rect = Some(hint_area);
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false }),
            input_area,
        );
        f.render_widget(
            Paragraph::new(hint_line).wrap(Wrap { trim: false }),
            hint_area,
        );
    }

    /// Render the bottom status bar
    pub fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let style = Style::default().fg(Color::Rgb(120, 120, 130));

        let cost_str = if let Some(cost) = self.session.token_usage.total_cost {
            format!("{:.3}", cost)
        } else {
            "0.00".to_string()
        };

        let (status_icon, status_color) = if self.agents.is_connected {
            ("\u{2713}", Color::Rgb(80, 200, 120)) // ✓
        } else {
            ("\u{2717}", Color::Rgb(220, 100, 100)) // ✗
        };

        let status_text = format!(
            "{} \u{2022} \u{2387} {} \u{2022} ${} \u{2022} {} queued | / commands \u{2022} @ mentions",
            self.model.as_deref().unwrap_or("claude"),
            self.active_branch,
            cost_str,
            self.session.message_queue.len()
        );

        let spans = vec![
            Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(status_color),
            ),
            Span::styled(status_text, style),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
}
