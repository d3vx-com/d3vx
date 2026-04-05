//! Batch inspector for parallel agent batches

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::ParallelChildStatus;
use crate::app::App;

impl App {
    pub(super) fn render_batch_detail(&self, f: &mut Frame, area: Rect, bg_color: Color) {
        f.render_widget(Block::default().style(Style::default().bg(bg_color)), area);

        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        let Some(batch) = self
            .agents
            .parallel_batches
            .values()
            .max_by_key(|batch| batch.started_at)
        else {
            let empty = Paragraph::new("No coordinated multi-agent batch is active.")
                .style(Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color))
                .wrap(Wrap { trim: true });
            f.render_widget(empty, inner);
            return;
        };

        let mut lines = Vec::new();
        lines.push(Line::from(vec![Span::styled(
            format!("Batch #{}", &batch.id[..batch.id.len().min(8)]),
            Style::default()
                .fg(self.ui.theme.brand)
                .bg(bg_color)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            batch.reasoning.clone(),
            Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
        )]));
        if batch.select_best {
            lines.push(Line::from(vec![
                Span::styled(
                    "selection: ",
                    Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                ),
                Span::styled(
                    batch
                        .selected_child_key
                        .clone()
                        .unwrap_or_else(|| "pending".to_string()),
                    Style::default().fg(self.ui.theme.brand).bg(bg_color),
                ),
            ]));
            if let Some(reason) = &batch.selection_reasoning {
                lines.push(Line::from(vec![Span::styled(
                    reason.clone(),
                    Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                )]));
            }
        }
        lines.push(Line::raw(""));

        for child in &batch.children {
            let status = match child.status {
                ParallelChildStatus::Pending => "pending",
                ParallelChildStatus::Running => "running",
                ParallelChildStatus::Completed => "completed",
                ParallelChildStatus::Failed => "failed",
                ParallelChildStatus::Cancelled => "cancelled",
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", status),
                    Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                ),
                Span::styled(
                    format!("{} ", child.specialist_role),
                    Style::default()
                        .fg(self.ui.theme.brand_secondary)
                        .bg(bg_color),
                ),
                Span::styled(
                    child.description.clone(),
                    Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                ),
            ]));
            if !child.depends_on.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        "    depends_on: ",
                        Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                    ),
                    Span::styled(
                        child.depends_on.join(", "),
                        Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                    ),
                ]));
            }
            if let Some(ownership) = &child.ownership {
                lines.push(Line::from(vec![
                    Span::styled(
                        "    ownership: ",
                        Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                    ),
                    Span::styled(
                        ownership.clone(),
                        Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                    ),
                ]));
            }
            if let Some(agent_id) = &child.agent_id {
                lines.push(Line::from(vec![
                    Span::styled(
                        "    agent: ",
                        Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                    ),
                    Span::styled(
                        &agent_id[..agent_id.len().min(8)],
                        Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                    ),
                ]));
            }
            if let Some(task_id) = &child.task_id {
                lines.push(Line::from(vec![
                    Span::styled(
                        "    task: ",
                        Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                    ),
                    Span::styled(
                        task_id.clone(),
                        Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                    ),
                ]));
            }
            if let Some(evaluation) = &child.evaluation {
                lines.push(Line::from(vec![
                    Span::styled(
                        "    score: ",
                        Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                    ),
                    Span::styled(
                        format!(
                            "{} (scope {} / tests {} / docs {} / conflict {} / files {})",
                            evaluation.total_score,
                            evaluation.scope_adherence,
                            evaluation.test_lint_outcome,
                            evaluation.docs_completeness,
                            evaluation.conflict_risk,
                            evaluation.changed_file_quality
                        ),
                        Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
                    ),
                ]));
                for note in evaluation.notes.iter().take(3) {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "      - ",
                            Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                        ),
                        Span::styled(
                            note.clone(),
                            Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                        ),
                    ]));
                }
            }
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .style(Style::default().bg(bg_color))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
    }
}
