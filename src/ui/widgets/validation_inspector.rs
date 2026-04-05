//! Validation Inspector Widget
//!
//! Renders validation summary in the inspector pane.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use crate::pipeline::validation_summary::{Confidence, ValidationSummary, ValidationUiSummary};

pub struct ValidationInspector<'a> {
    summary: Option<&'a ValidationSummary>,
}

impl<'a> ValidationInspector<'a> {
    pub fn new(summary: Option<&'a ValidationSummary>) -> Self {
        Self { summary }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let Some(summary) = self.summary else {
            self.render_empty(f, area);
            return;
        };

        let ui = summary.summary_for_ui();
        self.render_summary(f, area, &ui);
    }

    fn render_summary(&self, f: &mut Frame, area: Rect, ui: &ValidationUiSummary) {
        let (status_color, bg_color) = match ui.confidence {
            Confidence::None => (Color::Rgb(100, 100, 120), Color::Rgb(26, 26, 26)),
            Confidence::InProgress => (Color::Rgb(100, 150, 220), Color::Rgb(26, 30, 40)),
            Confidence::Low => (Color::Rgb(220, 100, 100), Color::Rgb(40, 20, 20)),
            Confidence::Medium => (Color::Rgb(220, 180, 60), Color::Rgb(35, 30, 20)),
            Confidence::High => (Color::Rgb(80, 200, 120), Color::Rgb(20, 40, 25)),
        };

        let block = Block::default()
            .title("Validation")
            .borders(ratatui::widgets::Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        f.render_widget(block, area);

        let mut lines = Vec::new();

        // Status line with confidence
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Rgb(150, 150, 160))),
            Span::styled(
                match ui.confidence {
                    Confidence::None => "NOT RUN",
                    Confidence::InProgress => "IN PROGRESS",
                    Confidence::Low => "LOW",
                    Confidence::Medium => "MEDIUM",
                    Confidence::High => "HIGH",
                },
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Pass/fail counts
        lines.push(Line::from(vec![
            Span::styled("Results: ", Style::default().fg(Color::Rgb(150, 150, 160))),
            Span::styled(
                format!("{}/{} passed", ui.passed, ui.total),
                if ui.failed > 0 {
                    Style::default().fg(Color::Rgb(220, 100, 100))
                } else {
                    Style::default().fg(Color::Rgb(80, 200, 120))
                },
            ),
            if ui.warnings > 0 {
                Span::styled(
                    format!(", {} warnings", ui.warnings),
                    Style::default().fg(Color::Rgb(220, 180, 60)),
                )
            } else {
                Span::raw("")
            },
        ]));

        // Check icons for type_check, test, lint
        lines.push(Line::from(vec![Span::styled(
            "Checks: ",
            Style::default().fg(Color::Rgb(150, 150, 160)),
        )]));

        let checks = [
            ("type", ui.type_check_passed),
            ("test", ui.test_passed),
            ("lint", ui.lint_passed),
        ];

        for (name, result) in checks {
            let (icon, color) = match result {
                Some(true) => ("[x]", Color::Rgb(80, 200, 120)),
                Some(false) => ("[!]", Color::Rgb(220, 100, 100)),
                None => ("[-]", Color::Rgb(80, 80, 100)),
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(name, Style::default().fg(Color::Rgb(180, 180, 190))),
            ]));
        }

        // Confidence score bar
        lines.push(Line::from(vec![Span::styled(
            format!("Confidence: {}%", ui.confidence_score),
            Style::default().fg(Color::Rgb(150, 150, 160)),
        )]));

        // Merge status
        let (merge_text, merge_color) = if ui.can_merge {
            ("MERGE READY", Color::Rgb(80, 200, 120))
        } else {
            ("MERGE BLOCKED", Color::Rgb(220, 100, 100))
        };

        lines.push(Line::from(vec![Span::raw("")]));
        lines.push(Line::from(vec![Span::styled(
            merge_text,
            Style::default()
                .fg(merge_color)
                .add_modifier(Modifier::BOLD),
        )]));

        // Duration
        if ui.duration_ms > 0 {
            lines.push(Line::from(vec![Span::styled(
                format!("Duration: {:.1}s", ui.duration_ms as f64 / 1000.0),
                Style::default().fg(Color::Rgb(100, 100, 120)),
            )]));
        }

        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(bg_color));

        f.render_widget(para, inner);
    }

    fn render_empty(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Validation")
            .borders(ratatui::widgets::Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        f.render_widget(block, area);

        let para = Paragraph::new("No validation data")
            .style(Style::default().fg(Color::Rgb(100, 100, 120)));
        f.render_widget(para, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creates_with_none() {
        let inspector = ValidationInspector::new(None);
        assert!(inspector.summary.is_none());
    }
}
