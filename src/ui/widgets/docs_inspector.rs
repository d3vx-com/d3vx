//! Docs Completeness Inspector Widget
//!
//! Renders docs completeness status in the inspector pane.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use crate::pipeline::docs_completeness::{DocsCompleteness, DocsStatus};

pub struct DocsInspector<'a> {
    completeness: Option<&'a DocsCompleteness>,
}

impl<'a> DocsInspector<'a> {
    pub fn new(completeness: Option<&'a DocsCompleteness>) -> Self {
        Self { completeness }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let Some(completeness) = self.completeness else {
            self.render_empty(f, area);
            return;
        };

        let (status_color, bg_color) = match completeness.status {
            DocsStatus::NotEvaluated => (Color::Rgb(100, 100, 120), Color::Rgb(26, 26, 26)),
            DocsStatus::NotRequired => (Color::Rgb(80, 200, 120), Color::Rgb(20, 40, 25)),
            DocsStatus::Complete => (Color::Rgb(80, 200, 120), Color::Rgb(20, 40, 25)),
            DocsStatus::Missing => (Color::Rgb(220, 100, 100), Color::Rgb(40, 20, 20)),
            DocsStatus::Partial => (Color::Rgb(220, 180, 60), Color::Rgb(35, 30, 20)),
        };

        let block = Block::default()
            .title("Docs")
            .borders(ratatui::widgets::Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        f.render_widget(block, area);

        let muted = Color::Rgb(150, 150, 160);
        let mut lines = Vec::new();

        // Status
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(muted)),
            Span::styled(
                self.status_display(completeness.status),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Required indicator
        lines.push(Line::from(vec![
            Span::styled("Required: ", Style::default().fg(muted)),
            Span::styled(
                if completeness.docs_required {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(if completeness.docs_required {
                    Color::Rgb(220, 180, 60)
                } else {
                    Color::Rgb(80, 200, 120)
                }),
            ),
        ]));

        // Signals
        if !completeness.signals.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Checks:",
                Style::default().fg(muted),
            )]));

            for signal in &completeness.signals {
                let icon = if signal.satisfied { "[x]" } else { "[ ]" };
                let color = if signal.satisfied {
                    Color::Rgb(80, 200, 120)
                } else {
                    Color::Rgb(220, 100, 100)
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        icon,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {} - ", signal.doc_type.display_name()),
                        Style::default().fg(Color::Rgb(180, 180, 190)),
                    ),
                    Span::styled(
                        signal.reason.clone(),
                        Style::default().fg(Color::Rgb(120, 120, 140)),
                    ),
                ]));
            }
        }

        // Merge status
        lines.push(Line::from(vec![Span::styled("", Color::Reset)]));

        let (merge_text, merge_color) = if completeness.can_merge() {
            ("MERGE READY", Color::Rgb(80, 200, 120))
        } else {
            ("MERGE BLOCKED", Color::Rgb(220, 100, 100))
        };

        lines.push(Line::from(vec![Span::styled(
            merge_text,
            Style::default()
                .fg(merge_color)
                .add_modifier(Modifier::BOLD),
        )]));

        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(bg_color));

        f.render_widget(para, inner);
    }

    fn status_display(&self, status: DocsStatus) -> &'static str {
        match status {
            DocsStatus::NotEvaluated => "NOT EVALUATED",
            DocsStatus::NotRequired => "NOT REQUIRED",
            DocsStatus::Complete => "COMPLETE",
            DocsStatus::Missing => "MISSING",
            DocsStatus::Partial => "PARTIAL",
        }
    }

    fn render_empty(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Docs")
            .borders(ratatui::widgets::Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        f.render_widget(block, area);

        let para = Paragraph::new("No docs evaluation")
            .style(Style::default().fg(Color::Rgb(100, 100, 120)));
        f.render_widget(para, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::docs_completeness::DocsCompletenessEvaluator;

    #[test]
    fn test_creates_with_none() {
        let inspector = DocsInspector::new(None);
        assert!(inspector.completeness.is_none());
    }

    #[test]
    fn test_not_required_case() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let result = evaluator.evaluate(&[], "format code");
        assert!(result.can_merge());
    }
}
