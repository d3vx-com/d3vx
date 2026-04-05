//! Input area and status bar rendering

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ui::icons;

impl App {
    /// Render input area (Timeline style)
    pub fn render_input(&mut self, f: &mut Frame, area: Rect) {
        let inner_area = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 0,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        let mut mode_spans = vec![Span::styled(
            "Mode ",
            Style::default().fg(self.ui.theme.ui.text_dim),
        )];
        for mode in crate::app::state::FocusMode::ALL {
            let active = self.ui.focus_mode == mode;
            mode_spans.push(Span::styled(
                format!(" {} ", mode.label()),
                if active {
                    Style::default()
                        .bg(self.ui.theme.brand)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .bg(Color::Rgb(34, 34, 40))
                        .fg(self.ui.theme.ui.text_muted)
                },
            ));
            mode_spans.push(Span::raw(" "));
        }
        mode_spans.push(Span::styled(
            "Ctrl+Tab cycle",
            Style::default().fg(self.ui.theme.ui.text_dim),
        ));

        let mut prompt = self.ui.input_buffer.clone();

        // Add vertical blinking indicator
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

        // Smart Highlighting
        let mut spans = vec![Span::styled(
            "\u{2192} ",
            Style::default().fg(Color::Rgb(80, 200, 120)),
        )];

        let tokens: Vec<&str> = prompt.split_inclusive(char::is_whitespace).collect();

        // If it's empty, show a subtle placeholder
        if tokens.is_empty() || (tokens.len() == 1 && tokens[0] == cursor_char) {
            spans.push(Span::styled(
                self.ui.focus_mode.hint(),
                Style::default().fg(Color::Rgb(100, 100, 110)),
            ));
            if tokens.len() == 1 && tokens[0] == cursor_char {
                spans.insert(1, Span::raw(cursor_char));
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

        // Render the text into the inner area with wrapping
        self.layout.last_mode_bar_rect = Some(chunks[0]);
        let mode_bar = Paragraph::new(Line::from(mode_spans)).wrap(Wrap { trim: false });
        let paragraph = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false });
        f.render_widget(Clear, area);
        f.render_widget(mode_bar, chunks[0]);
        f.render_widget(paragraph, chunks[1]);
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
            (icons::status::check(), Color::Rgb(80, 200, 120))
        } else {
            (icons::status::x(), Color::Rgb(220, 100, 100))
        };

        let status_text = format!(
            "{} \u{2022} {} {} \u{2022} ${} \u{2022} {} queued | / commands \u{2022} @ mentions",
            self.model.as_deref().unwrap_or("claude"),
            icons::git::branch(),
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
