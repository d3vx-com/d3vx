//! Task list rendering

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::ParallelChildStatus;
use crate::app::App;

impl App {
    fn latest_parallel_batch(&self) -> Option<&crate::app::state::ParallelBatchState> {
        self.agents
            .parallel_batches
            .values()
            .max_by_key(|batch| batch.started_at)
    }

    fn multi_agent_graph_lines(&self) -> Vec<Line<'_>> {
        let Some(batch) = self.latest_parallel_batch() else {
            return Vec::new();
        };

        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                format!("Multi-Agent Graph #{}", &batch.id[..batch.id.len().min(8)]),
                Style::default()
                    .fg(self.ui.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if batch.select_best { "  best-of-N" } else { "" },
                Style::default().fg(self.ui.theme.brand_secondary),
            ),
        ]));
        for child in &batch.children {
            let status = match child.status {
                ParallelChildStatus::Pending => "\u{25cb}",
                ParallelChildStatus::Running => "\u{25cf}",
                ParallelChildStatus::Completed => "\u{2713}",
                ParallelChildStatus::Failed => "\u{2717}",
                ParallelChildStatus::Cancelled => "-",
            };
            let winner_badge = if batch.selected_child_key.as_deref() == Some(child.key.as_str()) {
                "  [winner]"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} {} ", status, child.key),
                    Style::default().fg(self.ui.theme.brand_secondary),
                ),
                Span::styled(
                    child.description.clone(),
                    Style::default().fg(self.ui.theme.ui.text),
                ),
                Span::styled(
                    format!(" [{}]", child.specialist_role),
                    Style::default().fg(self.ui.theme.ui.text_dim),
                ),
                Span::styled(winner_badge, Style::default().fg(self.ui.theme.brand)),
            ]));
            if !child.depends_on.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        "   \u{2514}\u{2500} depends on ",
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                    Span::styled(
                        child.depends_on.join(", "),
                        Style::default().fg(self.ui.theme.ui.text),
                    ),
                ]));
            }
            if let Some(ownership) = &child.ownership {
                lines.push(Line::from(vec![
                    Span::styled(
                        "   \u{2514}\u{2500} owns ",
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                    Span::styled(
                        ownership.clone(),
                        Style::default().fg(self.ui.theme.ui.text),
                    ),
                ]));
            }
        }
        lines
    }

    pub fn render_task_list(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" TASK LIST ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.ui.theme.ui.border));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines: Vec<Line<'_>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "  Controls: ",
                Style::default().fg(self.ui.theme.ui.text_muted),
            ),
            Span::styled("j/k", Style::default().fg(self.ui.theme.brand_secondary)),
            Span::raw(" move  "),
            Span::styled("Space", Style::default().fg(self.ui.theme.brand_secondary)),
            Span::raw(" mark done  "),
            Span::styled("Enter", Style::default().fg(self.ui.theme.brand_secondary)),
            Span::raw(" open workspace"),
        ]));
        lines.push(Line::raw(""));

        let graph_lines = self.multi_agent_graph_lines();
        if !graph_lines.is_empty() {
            lines.extend(graph_lines);
            lines.push(Line::raw(""));
        }

        if self.task_view_tasks.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "No tasks available.",
                Style::default().fg(self.ui.theme.ui.text_dim),
            )]));
        } else {
            for (index, task) in self.task_view_tasks.iter().enumerate() {
                let selected = index == self.list_selected_task;
                let row_style = if selected {
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.ui.theme.ui.text)
                };
                let prefix = if selected { "\u{276f} " } else { "  " };
                let is_done = task.state == crate::store::task::TaskState::Done;
                let checkbox = if is_done { "[x]" } else { "[ ]" };
                let checkbox_style = if is_done {
                    Style::default().fg(self.ui.theme.state.success)
                } else {
                    Style::default().fg(self.ui.theme.ui.text_dim)
                };

                let mut title_style = row_style;
                if is_done {
                    title_style = title_style.add_modifier(Modifier::CROSSED_OUT).fg(self
                        .ui
                        .theme
                        .ui
                        .text_dim);
                }

                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(self.ui.theme.brand)),
                    Span::styled(format!("{} ", checkbox), checkbox_style),
                    Span::styled(
                        format!("{:<8} ", &task.id[..8.min(task.id.len())]),
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                    Span::styled(&task.title, title_style),
                    Span::raw(" "),
                    Span::styled(
                        format!("({})", task.state.to_string()),
                        Style::default().fg(self.ui.theme.ui.text_dim),
                    ),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
    }
}
