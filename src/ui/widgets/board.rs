//! Kanban Board Widget
//!
//! Displays tasks organized by their actual pipeline state.
//! Columns are backed by real TaskState values, not UI-only buckets.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::store::task::{ExecutionMode, TaskState};

/// A task displayed in the Kanban board
#[derive(Debug, Clone)]
pub struct KanbanTask {
    /// Unique task identifier
    pub id: String,
    /// Task title
    pub title: String,
    /// Current task state
    pub state: TaskState,
    /// Execution mode (shows as badge)
    pub execution_mode: Option<ExecutionMode>,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Merge ready status (None = not evaluated, Some(true) = ready, Some(false) = blocked)
    pub merge_ready: Option<bool>,
    /// Number of blocking issues (for display)
    pub blocking_count: usize,
    /// QA iteration (0 = not started)
    pub qa_iteration: u32,
}

/// Kanban column definition backed by real task states
#[derive(Debug, Clone)]
pub struct KanbanColumn {
    /// Display name
    pub name: &'static str,
    /// States that belong to this column
    pub states: &'static [TaskState],
    /// Column color
    pub color: Color,
}

impl KanbanColumn {
    /// Define the standard Kanban columns based on the blueprint
    pub const STANDARD_COLUMNS: &'static [Self] = &[
        Self {
            name: "To Do",
            states: &[TaskState::Backlog, TaskState::Queued],
            color: Color::Gray,
        },
        Self {
            name: "In Progress",
            states: &[
                TaskState::Research,
                TaskState::Plan,
                TaskState::Implement,
                TaskState::Validate,
                TaskState::Analyze,
                TaskState::AddNew,
                TaskState::Migrate,
                TaskState::RemoveOld,
                TaskState::Reproduce,
                TaskState::Investigate,
                TaskState::Fix,
                TaskState::Harden,
                TaskState::Preparing,
                TaskState::Spawning,
                TaskState::Prepare,
                TaskState::Test,
                TaskState::Execute,
                TaskState::Cleanup,
            ],
            color: Color::Blue,
        },
        Self {
            name: "Needs Attention",
            states: &[TaskState::Failed],
            color: Color::Rgb(220, 160, 50), // Amber
        },
        Self {
            name: "Review",
            states: &[TaskState::Review, TaskState::Docs, TaskState::Learn],
            color: Color::Yellow,
        },
        Self {
            name: "Done",
            states: &[TaskState::Done],
            color: Color::Green,
        },
    ];

    /// Check if a task belongs to this column
    pub fn contains_state(&self, state: &TaskState) -> bool {
        self.states.contains(state)
    }
}

/// Kanban board state
pub struct Board {
    /// All tasks to display
    pub tasks: Vec<KanbanTask>,
    /// Column definitions (can be customized)
    pub columns: &'static [KanbanColumn],
    /// Currently selected column index
    pub selected_col: usize,
    /// Currently selected task index within column
    pub selected_task: usize,
    /// Filter by project path
    pub project_filter: Option<String>,
    /// Optional graph-native summary of the latest multi-agent batch
    pub graph_summary: Vec<String>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// Create a new empty board with standard columns
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            columns: KanbanColumn::STANDARD_COLUMNS,
            selected_col: 0,
            selected_task: 0,
            project_filter: None,
            graph_summary: Vec::new(),
        }
    }

    /// Create a board with custom columns
    pub fn with_columns(columns: &'static [KanbanColumn]) -> Self {
        Self {
            tasks: Vec::new(),
            columns,
            selected_col: 0,
            selected_task: 0,
            project_filter: None,
            graph_summary: Vec::new(),
        }
    }

    /// Load tasks from the task store
    pub fn load_from_store(&mut self, tasks: Vec<KanbanTask>) {
        self.tasks = tasks;
    }

    pub fn set_graph_summary(&mut self, lines: Vec<String>) {
        self.graph_summary = lines;
    }

    /// Get tasks for a specific column
    fn tasks_for_column(&self, column_idx: usize) -> Vec<&KanbanTask> {
        let column = &self.columns[column_idx];
        self.tasks
            .iter()
            .filter(|t| column.contains_state(&t.state))
            .collect()
    }

    /// Get the currently selected task
    pub fn selected_task(&self) -> Option<&KanbanTask> {
        let tasks = self.tasks_for_column(self.selected_col);
        tasks.into_iter().nth(self.selected_task)
    }

    /// Move selection up within current column
    pub fn select_up(&mut self) {
        if self.selected_task > 0 {
            self.selected_task -= 1;
        }
    }

    /// Move selection down within current column
    pub fn select_down(&mut self) {
        let tasks = self.tasks_for_column(self.selected_col);
        if self.selected_task + 1 < tasks.len() {
            self.selected_task += 1;
        }
    }

    /// Move selection to previous column
    pub fn select_left(&mut self) {
        if self.selected_col > 0 {
            self.selected_col -= 1;
            self.selected_task = 0;
        }
    }

    /// Move selection to next column
    pub fn select_right(&mut self) {
        if self.selected_col + 1 < self.columns.len() {
            self.selected_col += 1;
            self.selected_task = 0;
        }
    }

    /// Render the board
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let has_graph = !self.graph_summary.is_empty();
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                if has_graph {
                    Constraint::Length((self.graph_summary.len() as u16).min(8) + 2)
                } else {
                    Constraint::Length(0)
                },
                Constraint::Min(4),
                Constraint::Length(1), // Footer
            ])
            .split(area);

        let graph_area = if has_graph {
            Some(main_chunks[0])
        } else {
            None
        };
        let board_area = if has_graph {
            main_chunks[1]
        } else {
            main_chunks[0]
        };
        let footer_area = if has_graph {
            main_chunks[2]
        } else {
            main_chunks[1]
        };

        if let Some(graph_area) = graph_area {
            let lines: Vec<Line<'_>> = self
                .graph_summary
                .iter()
                .map(|line| Line::from(Span::raw(line.clone())))
                .collect();
            let graph = Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(" Parallel Agents ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(graph, graph_area);
        }

        let num_cols = self.columns.len();
        let constraints: Vec<Constraint> = (0..num_cols)
            .map(|_| Constraint::Ratio(1, num_cols as u32))
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(board_area);

        for (i, column) in self.columns.iter().enumerate() {
            let is_selected = i == self.selected_col;
            let border_color = if is_selected {
                Color::Cyan
            } else {
                column.color
            };

            let _style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(column.color)
            };

            let column_tasks = self.tasks_for_column(i);
            let count = column_tasks.len();

            let title = format!("{} ({})", column.name, count);

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let mut lines = Vec::new();
            for (j, task) in column_tasks.iter().enumerate() {
                let is_task_selected = is_selected && j == self.selected_task;

                let task_style = if is_task_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Build task line with execution mode badge
                let mode_badge = match &task.execution_mode {
                    Some(ExecutionMode::Vex) => " ⏎",  // Background task
                    Some(ExecutionMode::Direct) => "", // Interactive is the default
                    Some(ExecutionMode::Auto) => "",
                    None => "",
                };

                // Build trust indicator
                let trust_indicator: String = match task.merge_ready {
                    Some(true) => "[✓]".to_string(),
                    Some(false) => format!("[✗{}]", task.blocking_count),
                    None => String::new(),
                };

                let priority_indicator = if task.priority > 0 {
                    "↑".repeat(task.priority.min(3) as usize)
                } else {
                    String::new()
                };

                // QA iteration indicator
                let qa_indicator = if task.qa_iteration > 0 {
                    format!("(Q{})", task.qa_iteration)
                } else {
                    String::new()
                };

                let task_text = if !priority_indicator.is_empty()
                    || !mode_badge.is_empty()
                    || !trust_indicator.is_empty()
                {
                    format!(
                        "{}{} {}{}{}",
                        trust_indicator, priority_indicator, task.title, mode_badge, qa_indicator
                    )
                } else {
                    format!("• {}", task.title)
                };

                // Color trust indicator
                let task_line = if task.merge_ready == Some(false) {
                    Line::from(Span::styled(
                        task_text,
                        task_style.fg(Color::Rgb(220, 100, 100)),
                    ))
                } else if task.merge_ready == Some(true) {
                    Line::from(Span::styled(
                        task_text,
                        task_style.fg(Color::Rgb(80, 200, 120)),
                    ))
                } else {
                    Line::from(Span::styled(task_text, task_style))
                };

                lines.push(task_line);
            }

            let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[i]);
        }

        // Render Footer
        let in_inbox = self.selected_col == 0;
        let mut hints = vec![
            Span::styled(" h/l ", Style::default().fg(Color::Cyan)),
            Span::raw("column "),
            Span::styled(" j/k ", Style::default().fg(Color::Cyan)),
            Span::raw("task "),
            Span::styled(" H/L ", Style::default().fg(Color::Cyan)),
            Span::raw("move state "),
        ];

        if in_inbox {
            hints.push(Span::styled(" a ", Style::default().fg(Color::Cyan)));
            hints.push(Span::raw("add task "));
        }

        hints.push(Span::styled(" Enter ", Style::default().fg(Color::Cyan)));
        hints.push(Span::raw("open "));
        hints.push(Span::styled(" Esc ", Style::default().fg(Color::Cyan)));
        hints.push(Span::raw("close "));

        let footer =
            Paragraph::new(Line::from(hints)).alignment(ratatui::layout::Alignment::Center);
        f.render_widget(footer, footer_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_state_mapping() {
        let columns = KanbanColumn::STANDARD_COLUMNS;

        // Backlog should be in To Do
        assert!(columns[0].contains_state(&TaskState::Backlog));
        assert!(columns[0].contains_state(&TaskState::Queued));

        // Research should be in In Progress
        assert!(columns[1].contains_state(&TaskState::Research));
        assert!(columns[1].contains_state(&TaskState::Implement));

        // Failed should be in Needs Attention
        assert!(columns[2].contains_state(&TaskState::Failed));

        // Review should be in Review
        assert!(columns[3].contains_state(&TaskState::Review));

        // Done should be in Done
        assert!(columns[4].contains_state(&TaskState::Done));
    }

    #[test]
    fn test_navigation() {
        let mut board = Board::new();
        board.tasks = vec![
            KanbanTask {
                id: "1".to_string(),
                title: "Task 1".to_string(),
                state: TaskState::Backlog,
                execution_mode: None,
                priority: 0,
                merge_ready: None,
                blocking_count: 0,
                qa_iteration: 0,
            },
            KanbanTask {
                id: "2".to_string(),
                title: "Task 2".to_string(),
                state: TaskState::Implement,
                execution_mode: Some(ExecutionMode::Vex),
                priority: 1,
                merge_ready: Some(true),
                blocking_count: 0,
                qa_iteration: 2,
            },
        ];

        // Initial state
        assert_eq!(board.selected_col, 0);
        assert_eq!(board.selected_task, 0);

        // Navigate right to next column
        board.select_right();
        assert_eq!(board.selected_col, 1);

        // Navigate left back
        board.select_left();
        assert_eq!(board.selected_col, 0);
    }
}
