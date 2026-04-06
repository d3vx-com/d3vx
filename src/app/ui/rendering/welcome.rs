//! Welcome banner rendering

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::config::check_onboarding_status;
use crate::ui::symbols::STATUS;

impl App {
    /// Render the welcome banner
    pub fn render_welcome(&self, f: &mut Frame, area: Rect) {
        let brand_color = self.ui.theme.brand;
        let dim_color = self.ui.theme.ui.text_dim;
        let secondary_color = self.ui.theme.brand_secondary;
        let error_color = self.ui.theme.state.error;

        let is_vex_active = !self.background_active_tasks.is_empty();
        let onboarding = check_onboarding_status();

        // Mascot with blinking eye animation
        let blink_frame = self.animation_frame % 16;
        let (eye_left, eye_right) = match blink_frame {
            0..=7 => ("\u{2580}", "\u{2580}"),
            8..=11 => ("\u{2500}", "\u{2500}"),
            12..=15 => ("\u{2580}", "\u{2500}"),
            _ => ("\u{2580}", "\u{2580}"),
        };

        let is_ultra_narrow = area.width < 60;

        let mut lines: Vec<Line<'_>> = Vec::new();

        // Full mascot with blinking eye animation (4 lines)
        let mascot_lines = vec![
            "  \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}  ".to_string(),
            format!("  \u{2588} {} {} \u{2588}  ", eye_left, eye_right),
            " \u{2580}\u{2588}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2588}\u{2580} "
                .to_string(),
            "   \u{2588}   \u{2588}   ".to_string(),
        ];

        if !is_ultra_narrow {
            for (i, line) in mascot_lines.iter().enumerate() {
                let color = if i == 1 { brand_color } else { dim_color };
                let mut spans = vec![Span::styled(
                    line,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )];

                // Add "VEX_MODE" tag next to the robot head
                if i == 1 && is_vex_active {
                    spans.push(Span::styled(
                        " [ VEX_MODE ]",
                        Style::default()
                            .fg(secondary_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                lines.push(Line::from(spans));
            }
            lines.push(Line::raw(""));
        }

        // Setup or connected state
        if !self.agents.is_connected || onboarding.is_first_run {
            lines.push(Line::from(vec![Span::styled(
                "  setup required",
                Style::default().bg(error_color).fg(Color::Black),
            )]));
            lines.push(Line::raw(""));

            if let Some(ref hint) = self.session.init_hint {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", hint),
                    Style::default().fg(secondary_color),
                )]));
            }

            if let Some(provider) = &onboarding.missing_provider {
                let env_var = &onboarding.provider_api_key_env;
                if !env_var.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        format!("  {}=...", env_var),
                        Style::default().fg(dim_color),
                    )]));
                } else if provider == "ollama" {
                    lines.push(Line::from(vec![Span::styled(
                        "  ollama.ai — install, then: ollama serve",
                        Style::default().fg(dim_color),
                    )]));
                }
            }

            if onboarding.needs_api_key_setup {
                lines.push(Line::from(vec![Span::styled(
                    "  API key missing — /setup for instructions, or quit & run d3vx setup",
                    Style::default().fg(error_color),
                )]));
            }

            lines.push(Line::from(vec![
                Span::styled("  /doctor", Style::default().fg(brand_color)),
                Span::styled(" or ", Style::default().fg(dim_color)),
                Span::styled("/setup", Style::default().fg(brand_color)),
            ]));
        } else {
            // Connected - minimal status
            let status = if self.agents.is_connected {
                STATUS.success
            } else {
                STATUS.error
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", status),
                    Style::default().fg(self.ui.theme.state.success),
                ),
                Span::styled(
                    self.model.as_deref().unwrap_or("claude"),
                    Style::default().fg(dim_color),
                ),
            ]));

            if let Some(cost) = self.session.token_usage.total_cost {
                lines.push(Line::from(vec![Span::styled(
                    format!("  ${:.3}", cost),
                    Style::default().fg(brand_color),
                )]));
            }

            lines.push(Line::raw(""));
            lines.push(Line::from(vec![Span::styled(
                "  type a message or /help",
                Style::default().fg(dim_color),
            )]));
        }

        let paragraph = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Left);
        f.render_widget(paragraph, area);
    }
}
