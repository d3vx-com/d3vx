//! Trust Panel Widget
//!
//! Renders unified merge readiness, blocking reasons, and trust signals in a compact format.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::pipeline::merge_gate::{MergeReadiness, MergeSource};
use crate::pipeline::qa_loop::{QAState, QAStatus};
use crate::pipeline::review_summary::ReviewSummary;
use crate::pipeline::validation_summary::{Confidence, ValidationSummary};

pub struct TrustPanel<'a> {
    merge_readiness: Option<&'a MergeReadiness>,
    review_summary: Option<&'a ReviewSummary>,
    validation_summary: Option<&'a ValidationSummary>,
    qa_status: Option<&'a QAStatus>,
}

impl<'a> TrustPanel<'a> {
    pub fn new(
        merge_readiness: Option<&'a MergeReadiness>,
        review_summary: Option<&'a ReviewSummary>,
        validation_summary: Option<&'a ValidationSummary>,
        qa_status: Option<&'a QAStatus>,
    ) -> Self {
        Self {
            merge_readiness,
            review_summary,
            validation_summary,
            qa_status,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Readiness ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Rgb(26, 26, 26)));

        f.render_widget(block, area);

        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.extend(self.render_merge_header());
        lines.push(Line::raw(""));

        lines.extend(self.render_qa_state());
        lines.push(Line::raw(""));

        lines.extend(self.render_signals());

        // Detailed validation breakdown (check-by-check)
        if let Some(validation) = self.validation_summary {
            lines.push(Line::raw(""));
            lines.extend(self.render_validation_detail(validation));
        }

        // Detailed review findings (individual issues)
        if let Some(review) = self.review_summary {
            if !review.findings.is_empty() {
                lines.push(Line::raw(""));
                lines.extend(self.render_findings_detail(review));
            }
        }

        if let Some(readiness) = self.merge_readiness {
            if !readiness.reasons.is_empty() {
                lines.push(Line::raw(""));
                lines.extend(self.render_blocking_reasons(readiness));
            }

            if !readiness.warnings.is_empty() {
                lines.push(Line::raw(""));
                lines.extend(self.render_warnings(readiness));
            }
        }

        let para = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(Color::Rgb(20, 20, 22)));

        f.render_widget(para, inner);
    }

    fn render_merge_header(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        let (text, text_color) = match self.merge_readiness {
            Some(r) if r.ready => ("✓ MERGE READY", Color::Rgb(80, 200, 120)),
            Some(r) if r.blocked => ("✗ MERGE BLOCKED", Color::Rgb(220, 100, 100)),
            Some(r) if r.reasons.is_empty() && r.warnings.is_empty() => {
                ("? Not evaluated yet", Color::Rgb(150, 150, 160))
            }
            _ => ("? Waiting for task data", Color::Rgb(150, 150, 160)),
        };

        lines.push(Line::from(vec![Span::styled(
            text,
            Style::default().fg(text_color).add_modifier(Modifier::BOLD),
        )]));

        if let Some(r) = self.merge_readiness {
            if !r.summary.is_empty() && r.summary != "Ready to merge" {
                lines.push(Line::from(vec![Span::styled(
                    &r.summary,
                    Style::default().fg(Color::Rgb(130, 130, 140)),
                )]));
            }
        }

        lines
    }

    fn render_qa_state(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        if let Some(status) = self.qa_status {
            let state_text = match status.state {
                QAState::Pending => "pending",
                QAState::InReview => "in review",
                QAState::AwaitingFix => "awaiting fix",
                QAState::InFix => "fixing",
                QAState::ReReview => "re-review",
                QAState::Approved => "approved",
                QAState::Escalated => "escalated",
            };

            let state_color = match status.state {
                QAState::Approved => Color::Rgb(80, 200, 120),
                QAState::Escalated => Color::Rgb(220, 100, 100),
                QAState::InReview | QAState::ReReview => Color::Rgb(100, 150, 220),
                QAState::AwaitingFix | QAState::InFix => Color::Rgb(220, 180, 60),
                QAState::Pending => Color::Rgb(100, 100, 120),
            };

            let iter_text = if status.max_retries > 1 {
                format!(" (attempt {}/{})", status.iteration, status.max_retries)
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::styled("QA: ", Style::default().fg(Color::Rgb(150, 150, 160))),
                Span::styled(state_text, Style::default().fg(state_color)),
                Span::styled(iter_text, Style::default().fg(Color::Rgb(100, 100, 120))),
            ]));

            if status.pending_fixes > 0 {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {} fix(es) pending", status.pending_fixes),
                    Style::default().fg(Color::Rgb(220, 180, 60)),
                )]));
            }
        }

        lines
    }

    fn render_signals(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Checks",
            Style::default()
                .fg(Color::Rgb(150, 150, 160))
                .add_modifier(Modifier::BOLD),
        )]));

        lines.extend(self.render_review_signal());
        lines.extend(self.render_validation_signal());
        lines.extend(self.render_docs_signal());

        lines
    }

    fn render_review_signal(&self) -> Vec<Line<'_>> {
        let (icon, label, detail) = if let Some(readiness) = self.merge_readiness {
            if let Some(signal) = &readiness.signals.review {
                let icon = if signal.ready { "[x]" } else { "[!]" };
                let detail = signal
                    .details
                    .as_ref()
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default();
                (icon, "Review", format!("{}{}", signal.status, detail))
            } else {
                ("[-]", "Review", "not run".to_string())
            }
        } else if let Some(review) = self.review_summary {
            let ready = review.status == crate::pipeline::review_summary::ReviewStatus::Approved;
            let blocking_count = review.blocking_findings.len();
            let detail = if ready {
                "passed".to_string()
            } else if blocking_count > 0 {
                format!("{} blocking issue(s)", blocking_count)
            } else {
                format!("{} finding(s)", review.findings.len())
            };
            (if ready { "[x]" } else { "[!]" }, "Review", detail)
        } else {
            ("[-]", "Review", "no data".to_string())
        };

        let color = match icon {
            "[x]" => Color::Rgb(80, 200, 120),
            "[!]" => Color::Rgb(220, 100, 100),
            _ => Color::Rgb(100, 100, 120),
        };

        vec![Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}: ", label),
                Style::default().fg(Color::Rgb(180, 180, 190)),
            ),
            Span::styled(detail, Style::default().fg(Color::Rgb(140, 140, 150))),
        ])]
    }

    fn render_validation_signal(&self) -> Vec<Line<'_>> {
        let (icon, label, detail) = if let Some(readiness) = self.merge_readiness {
            if let Some(signal) = &readiness.signals.validation {
                let icon = if signal.ready { "[x]" } else { "[!]" };
                let detail = signal
                    .details
                    .as_ref()
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default();
                (icon, "Validation", format!("{}{}", signal.status, detail))
            } else {
                ("[-]", "Validation", "not run".to_string())
            }
        } else if let Some(validation) = self.validation_summary {
            let icon = match validation.confidence {
                Confidence::High => "[x]",
                Confidence::Medium => "[~]",
                Confidence::Low => "[!]",
                Confidence::InProgress => "[...]",
                Confidence::None => "[-]",
            };
            (
                icon,
                "Validation",
                format!("{}/{} passed", validation.passed, validation.total),
            )
        } else {
            ("[-]", "Validation", "no data".to_string())
        };

        let color = match icon {
            "[x]" => Color::Rgb(80, 200, 120),
            "[!]" => Color::Rgb(220, 100, 100),
            "[~]" => Color::Rgb(220, 180, 60),
            _ => Color::Rgb(100, 100, 120),
        };

        vec![Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}: ", label),
                Style::default().fg(Color::Rgb(180, 180, 190)),
            ),
            Span::styled(detail, Style::default().fg(Color::Rgb(140, 140, 150))),
        ])]
    }

    fn render_docs_signal(&self) -> Vec<Line<'_>> {
        let (icon, label, detail) = if let Some(readiness) = self.merge_readiness {
            if let Some(signal) = &readiness.signals.docs {
                let icon = if signal.ready { "[x]" } else { "[!]" };
                let detail = signal
                    .details
                    .as_ref()
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_else(|| signal.status.clone());
                (icon, "Docs", detail)
            } else {
                ("[-]", "Docs", "not required".to_string())
            }
        } else {
            ("[-]", "Docs", "no data".to_string())
        };

        let color = match icon {
            "[x]" => Color::Rgb(80, 200, 120),
            "[!]" => Color::Rgb(220, 100, 100),
            _ => Color::Rgb(100, 100, 120),
        };

        vec![Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}: ", label),
                Style::default().fg(Color::Rgb(180, 180, 190)),
            ),
            Span::styled(detail, Style::default().fg(Color::Rgb(140, 140, 150))),
        ])]
    }

    /// Render per-check validation breakdown (type-check, test, lint)
    fn render_validation_detail(&self, validation: &ValidationSummary) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Checks detail",
            Style::default()
                .fg(Color::Rgb(150, 150, 160))
                .add_modifier(Modifier::BOLD),
        )]));

        let ui = validation.summary_for_ui();

        let checks = [
            ("Type check", ui.type_check_passed),
            ("Tests", ui.test_passed),
            ("Lint", ui.lint_passed),
        ];

        for (name, result) in checks {
            let (icon, color) = match result {
                Some(true) => ("pass", Color::Rgb(80, 200, 120)),
                Some(false) => ("fail", Color::Rgb(220, 100, 100)),
                None => ("—", Color::Rgb(80, 80, 100)),
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:>4} ", icon), Style::default().fg(color)),
                Span::styled(name, Style::default().fg(Color::Rgb(180, 180, 190))),
            ]));
        }

        // Overall pass rate
        if validation.total > 0 {
            let rate = validation.passed * 100 / validation.total;
            let rate_color = if rate >= 100 {
                Color::Rgb(80, 200, 120)
            } else if rate >= 50 {
                Color::Rgb(220, 180, 60)
            } else {
                Color::Rgb(220, 100, 100)
            };
            lines.push(Line::from(vec![Span::styled(
                format!("  {}/{} passed ", validation.passed, validation.total),
                Style::default().fg(rate_color),
            )]));
        }

        lines
    }

    /// Render individual review findings
    fn render_findings_detail(&self, review: &ReviewSummary) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Findings",
            Style::default()
                .fg(Color::Rgb(150, 150, 160))
                .add_modifier(Modifier::BOLD),
        )]));

        use crate::pipeline::review_summary::{FindingCategory, ReviewSeverity};

        for finding in &review.findings {
            let severity_icon = match finding.severity {
                ReviewSeverity::Critical => "!!!",
                ReviewSeverity::High => " !!",
                ReviewSeverity::Medium => "  !",
                ReviewSeverity::Low => "  o",
            };

            let severity_color = match finding.severity {
                ReviewSeverity::Critical => Color::Rgb(220, 60, 60),
                ReviewSeverity::High => Color::Rgb(220, 140, 60),
                ReviewSeverity::Medium => Color::Rgb(220, 180, 60),
                ReviewSeverity::Low => Color::Rgb(100, 140, 100),
            };

            let resolved = if finding.resolved { "[x]" } else { "[ ]" };

            let category = match finding.category {
                FindingCategory::Correctness => "correctness",
                FindingCategory::Security => "security",
                FindingCategory::Performance => "perf",
                FindingCategory::Maintainability => "maintain",
                FindingCategory::Coverage => "coverage",
                FindingCategory::Breaking => "breaking",
                FindingCategory::Risk => "risk",
                FindingCategory::Documentation => "docs",
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", resolved),
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                ),
                Span::styled(
                    format!("{} ", severity_icon),
                    Style::default().fg(severity_color),
                ),
                Span::styled(
                    finding.title.clone(),
                    Style::default().fg(Color::Rgb(220, 220, 230)),
                ),
            ]));

            // Location and category on second line
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

            lines.push(Line::from(vec![
                Span::raw("       "),
                Span::styled(category, Style::default().fg(Color::Rgb(80, 140, 200))),
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
            ]));
        }

        lines
    }

    fn render_blocking_reasons<'b>(&self, readiness: &'b MergeReadiness) -> Vec<Line<'b>> {
        let mut lines: Vec<Line<'b>> = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Blocking",
            Style::default()
                .fg(Color::Rgb(150, 150, 160))
                .add_modifier(Modifier::BOLD),
        )]));

        for reason in &readiness.reasons {
            let source_tag = match reason.source {
                MergeSource::Review => "[REV]",
                MergeSource::Validation => "[VAL]",
                MergeSource::Docs => "[DOC]",
            };

            let source_color = match reason.source {
                MergeSource::Review => Color::Rgb(220, 140, 60),
                MergeSource::Validation => Color::Rgb(100, 150, 220),
                MergeSource::Docs => Color::Rgb(140, 100, 200),
            };

            lines.push(Line::from(vec![
                Span::styled("  [!] ", Style::default().fg(Color::Rgb(220, 100, 100))),
                Span::styled(source_tag, Style::default().fg(source_color)),
                Span::raw(" "),
                Span::styled(
                    &reason.message,
                    Style::default().fg(Color::Rgb(200, 200, 210)),
                ),
            ]));
        }

        lines
    }

    fn render_warnings<'b>(&self, readiness: &'b MergeReadiness) -> Vec<Line<'b>> {
        let mut lines: Vec<Line<'b>> = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Warnings",
            Style::default()
                .fg(Color::Rgb(150, 150, 160))
                .add_modifier(Modifier::BOLD),
        )]));

        for warning in &readiness.warnings {
            lines.push(Line::from(vec![
                Span::styled("  [~] ", Style::default().fg(Color::Rgb(220, 180, 60))),
                Span::styled(
                    &warning.message,
                    Style::default().fg(Color::Rgb(180, 180, 190)),
                ),
            ]));
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creates_with_none() {
        let panel = TrustPanel::new(None, None, None, None);
        assert!(panel.merge_readiness.is_none());
    }

    #[test]
    fn test_merge_ready_state() {
        use crate::pipeline::merge_gate::MergeGate;
        let gate = MergeGate::with_defaults();
        let readiness = gate.evaluate(None, None, None);
        let panel = TrustPanel::new(Some(&readiness), None, None, None);
        assert!(panel.merge_readiness.is_some());
    }

    #[test]
    fn test_trust_panel_with_blocked_readiness() {
        use crate::pipeline::merge_gate::{MergeBlockingReason, MergeSignals};

        let readiness = MergeReadiness::blocked(
            vec![
                MergeBlockingReason::review("SECURITY", "SQL Injection found"),
                MergeBlockingReason::validation("VALIDATION_FAILED", "Tests failing"),
            ],
            vec![],
            MergeSignals::default(),
        );

        let panel = TrustPanel::new(Some(&readiness), None, None, None);
        assert!(!panel.merge_readiness.unwrap().ready);
    }

    #[test]
    fn test_trust_panel_readiness_serialization() {
        use crate::pipeline::merge_gate::MergeGate;
        let gate = MergeGate::with_defaults();
        let readiness = gate.evaluate(None, None, None);

        let json = serde_json::to_string(&readiness).unwrap();
        let deserialized: MergeReadiness = serde_json::from_str(&json).unwrap();

        assert_eq!(readiness.ready, deserialized.ready);
        assert_eq!(readiness.summary, deserialized.summary);
    }
}
