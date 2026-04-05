//! Shimmer Animation Utility
//!
//! Sweeps a highlight across text to create a high-end animated effect.

use crate::ui::theme::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct Shimmer<'a> {
    text: &'a str,
    frame: usize,
    theme: Theme,
}

impl<'a> Shimmer<'a> {
    pub fn new(text: &'a str, frame: usize, theme: Theme) -> Self {
        Self { text, frame, theme }
    }

    /// Build a Line with the shimmer effect applied
    pub fn build(self) -> Line<'a> {
        let mut spans = Vec::new();
        let width = self.text.chars().count();
        if width == 0 {
            return Line::from("");
        }

        // Shimmer moves across the text. We use the frame to determine the center.
        // Speed control: slower movement
        let shimmer_center = (self.frame / 2) % (width + 10);
        let shimmer_width = 5;

        for (i, c) in self.text.chars().enumerate() {
            let distance = if i > shimmer_center {
                i.saturating_sub(shimmer_center)
            } else {
                shimmer_center.saturating_sub(i)
            };

            let style = if distance < shimmer_width {
                // High brightness area
                let intensity = (shimmer_width - distance) as f32 / shimmer_width as f32;
                if intensity > 0.8 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if intensity > 0.4 {
                    Style::default()
                        .fg(self.theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.brand)
                }
            } else {
                Style::default().fg(self.theme.ui.text_dim)
            };

            spans.push(Span::styled(c.to_string(), style));
        }

        Line::from(spans)
    }
}
