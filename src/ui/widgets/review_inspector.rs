//! Review Inspector Widget
//!
//! Renders review summary and findings in the inspector pane.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::pipeline::review_summary::{
    FindingCategory, ReviewFinding, ReviewSeverity, ReviewStatus, ReviewSummary,
};
use crate::ui::theme::Theme;

pub struct ReviewInspector<'a> {
    review: Option<&'a ReviewSummary>,
    #[allow(dead_code)]
    theme: &'a Theme,
}

impl<'a> ReviewInspector<'a> {
    pub fn new(review: Option<&'a ReviewSummary>, theme: &'a Theme) -> Self {
        Self { review, theme }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let Some(review) = self.review else {
            self.render_empty(f, area);
            return;
        };

        let _block = Block::default()
            .title("Review")
            .borders(ratatui::widgets::Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Status header
                Constraint::Min(5),    // Findings list
                Constraint::Length(3), // Summary/merge status
            ])
            .split(inner);

        self.render_status_header(f, chunks[0], review);
        self.render_findings(f, chunks[1], review);
        self.render_merge_status(f, chunks[2], review);
    }

    fn render_status_header(&self, f: &mut Frame, area: Rect, review: &ReviewSummary) {
        let (status_text, status_color) = match review.status {
            ReviewStatus::Pending => ("PENDING", Color::Rgb(180, 180, 100)),
            ReviewStatus::InProgress => ("IN PROGRESS", Color::Rgb(100, 150, 220)),
            ReviewStatus::Approved => ("APPROVED", Color::Rgb(80, 200, 120)),
            ReviewStatus::Rejected => ("REJECTED", Color::Rgb(220, 100, 100)),
            ReviewStatus::Skipped => ("SKIPPED", Color::Rgb(100, 100, 120)),
        };

        let severity_counts = review.count_by_severity();
        let counts_text = format!(
            " {}critical {}high {}medium {}low",
            severity_counts[0], severity_counts[1], severity_counts[2], severity_counts[3]
        );

        // Recommended action based on review status
        let (action_text, action_color) = match review.status {
            ReviewStatus::Pending => (
                "Waiting for review to complete...",
                Color::Rgb(180, 180, 100),
            ),
            ReviewStatus::InProgress => ("Review in progress...", Color::Rgb(100, 150, 220)),
            ReviewStatus::Approved => (
                "Safe to approve — no blocking issues found",
                Color::Rgb(80, 200, 120),
            ),
            ReviewStatus::Rejected => {
                if review.blocking_findings.is_empty() {
                    (
                        "Issues found — review details below",
                        Color::Rgb(220, 180, 60),
                    )
                } else {
                    (
                        "Blocking issues found — resolve before approving",
                        Color::Rgb(220, 100, 100),
                    )
                }
            }
            ReviewStatus::Skipped => ("Review was skipped", Color::Rgb(100, 100, 120)),
        };

        let lines = vec![
            Line::from(vec![Span::styled(
                action_text,
                Style::default()
                    .fg(action_color)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Rgb(150, 150, 160))),
                Span::styled(
                    status_text,
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(counts_text, Style::default().fg(Color::Rgb(120, 120, 130))),
            ]),
            Line::from(vec![
                Span::styled("Findings: ", Style::default().fg(Color::Rgb(150, 150, 160))),
                Span::styled(
                    format!("{}", review.findings.len()),
                    Style::default().fg(Color::Rgb(200, 200, 210)),
                ),
                if review.blocking_findings.is_empty() {
                    Span::raw("")
                } else {
                    Span::styled(
                        format!(" ({} blocking)", review.blocking_findings.len()),
                        Style::default().fg(Color::Rgb(220, 100, 100)),
                    )
                },
            ]),
        ];

        let para = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }

    fn render_findings(&self, f: &mut Frame, area: Rect, review: &ReviewSummary) {
        if review.findings.is_empty() {
            let para =
                Paragraph::new("No findings").style(Style::default().fg(Color::Rgb(100, 100, 120)));
            f.render_widget(para, area);
            return;
        }

        let items: Vec<ListItem> = review
            .findings
            .iter()
            .map(|finding| self.finding_to_list_item(finding))
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(ratatui::widgets::Borders::BOTTOM)
                .style(Style::default()),
        );

        f.render_widget(list, area);
    }

    fn finding_to_list_item<'f>(&self, finding: &'f ReviewFinding) -> ListItem<'f> {
        let severity_icon = match finding.severity {
            ReviewSeverity::Critical => "!!!",
            ReviewSeverity::High => "!!",
            ReviewSeverity::Medium => "!",
            ReviewSeverity::Low => "o",
        };

        let severity_color = match finding.severity {
            ReviewSeverity::Critical => Color::Rgb(220, 60, 60),
            ReviewSeverity::High => Color::Rgb(220, 140, 60),
            ReviewSeverity::Medium => Color::Rgb(220, 180, 60),
            ReviewSeverity::Low => Color::Rgb(100, 140, 100),
        };

        let resolved_icon = if finding.resolved { "[x]" } else { "[ ]" };

        let category_tag = match finding.category {
            FindingCategory::Correctness => "[CORR]",
            FindingCategory::Security => "[SEC]",
            FindingCategory::Performance => "[PERF]",
            FindingCategory::Maintainability => "[MAIN]",
            FindingCategory::Coverage => "[COV]",
            FindingCategory::Breaking => "[BRK]",
            FindingCategory::Risk => "[RISK]",
            FindingCategory::Documentation => "[DOCS]",
        };

        let location = finding
            .location
            .as_ref()
            .map(|l| {
                if let Some(line) = l.line {
                    format!("{}:{}", l.file, line)
                } else {
                    l.file.clone()
                }
            })
            .unwrap_or_default();

        let content = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", resolved_icon),
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                ),
                Span::styled(
                    format!("{} ", severity_icon),
                    Style::default().fg(severity_color),
                ),
                Span::styled(
                    finding.title.as_str(),
                    Style::default().fg(Color::Rgb(220, 220, 230)).add_modifier(
                        if !finding.resolved && finding.severity.blocks_merge() {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        },
                    ),
                ),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(category_tag, Style::default().fg(Color::Rgb(80, 140, 200))),
                if !location.is_empty() {
                    Span::styled(
                        format!(" {}", location),
                        Style::default().fg(Color::Rgb(100, 120, 140)),
                    )
                } else {
                    Span::raw("")
                },
                if let Some(suggestion) = &finding.suggestion {
                    Span::styled(
                        format!(" → {}", suggestion),
                        Style::default().fg(Color::Rgb(100, 180, 140)),
                    )
                } else {
                    Span::raw("")
                },
            ]),
        ];

        ListItem::new(content)
    }

    fn render_merge_status(&self, f: &mut Frame, area: Rect, review: &ReviewSummary) {
        let (status_text, status_color, bg_color) = if review.is_merge_ready() {
            (
                "MERGE READY",
                Color::Rgb(80, 200, 120),
                Color::Rgb(20, 40, 25),
            )
        } else {
            (
                "MERGE BLOCKED",
                Color::Rgb(220, 100, 100),
                Color::Rgb(40, 20, 20),
            )
        };

        let lines = vec![Line::from(vec![
            Span::styled("Merge: ", Style::default().fg(Color::Rgb(150, 150, 160))),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];

        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(bg_color));
        f.render_widget(para, area);
    }

    fn render_empty(&self, f: &mut Frame, area: Rect) {
        let para = Paragraph::new("No review data available")
            .style(Style::default().fg(Color::Rgb(100, 100, 120)));
        f.render_widget(para, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::review_summary::ReviewSummary;

    fn make_test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn test_review_inspector_creates() {
        let theme = make_test_theme();
        let inspector = ReviewInspector::new(None, &theme);
        assert!(inspector.review.is_none());
    }

    #[test]
    fn test_review_summary_merge_ready() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.status = ReviewStatus::Approved;
        assert!(review.is_merge_ready());
    }

    #[test]
    fn test_review_summary_merge_blocked() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.merge_blocked = true;
        assert!(!review.is_merge_ready());
    }
}
