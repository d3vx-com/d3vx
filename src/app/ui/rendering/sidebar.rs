//! Sidebar rendering (board/list modes)

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use crate::app::state::WorkspaceType;
use crate::app::App;
use crate::pipeline::UnifiedTrustData;
use crate::ui::icons;

impl App {
    /// Render the unified minimalist sidebar (Navigator + Inspector)
    pub fn render_sidebar(&mut self, f: &mut Frame, area: Rect) {
        let sidebar_bg = Color::Rgb(26, 26, 26);
        let text_color = Color::Rgb(200, 200, 210);
        let muted_color = Color::Rgb(100, 100, 110);
        let brand_color = self.ui.theme.brand;

        let block = Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(sidebar_bg).fg(text_color));

        f.render_widget(Clear, area);
        f.render_widget(block, area);

        // Add padding to the inner content
        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        });

        let mut lines: Vec<Line<'_>> = Vec::new();
        self.layout.sidebar_agent_rows.clear();

        // Active Agents (minimal, one-liners)
        let agents = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.subagents.list())
        });
        let active_agents: Vec<_> = agents
            .iter()
            .filter(|a| a.status == crate::agent::SubAgentStatus::Running)
            .collect();

        if !active_agents.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                format!("Agents: {}", active_agents.len()),
                Style::default().fg(text_color).add_modifier(Modifier::BOLD),
            )]));
            self.layout.sidebar_agent_rows.push(usize::MAX);

            for (idx, agent) in active_agents.iter().take(3).enumerate() {
                let agent_idx = self
                    .agents
                    .inline_agents
                    .iter()
                    .position(|a| a.id == agent.id)
                    .unwrap_or(idx);
                self.layout.sidebar_agent_rows.push(agent_idx);

                let prefix = if (self.animation_frame / 4) % 2 == 0 {
                    ">"
                } else {
                    "*"
                };
                let is_expanded = self
                    .agents
                    .inline_agents
                    .iter()
                    .any(|a| a.id == agent.id && a.expanded);
                let expand_indicator = if is_expanded { "[-]" } else { "[+]" };

                lines.push(Line::from(vec![
                    Span::styled(format!(" {} ", prefix), Style::default().fg(brand_color)),
                    Span::styled(expand_indicator, Style::default().fg(muted_color)),
                    Span::raw(" "),
                    Span::styled(
                        if agent.task.len() > 14 {
                            format!("{}..", &agent.task[..12])
                        } else {
                            agent.task.clone()
                        },
                        Style::default().fg(text_color),
                    ),
                ]));
            }

            if active_agents.len() > 3 {
                lines.push(Line::from(vec![Span::styled(
                    format!("  +{} more", active_agents.len() - 3),
                    Style::default().fg(muted_color),
                )]));
            }
            lines.push(Line::raw(""));
        }

        // Workspaces (minimal list)
        if !self.workspaces.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Workspaces",
                Style::default().fg(text_color).add_modifier(Modifier::BOLD),
            )]));

            for (index, task) in self.workspaces.iter().enumerate().take(5) {
                let is_selected = index == self.workspace_selected_index;
                let icon = match task.workspace_type {
                    WorkspaceType::Anchor => icons::files::folder(),
                    WorkspaceType::Satellite => icons::files::folder_open(),
                    WorkspaceType::SubAgent => icons::dev::code(),
                };
                let icon_color = if is_selected {
                    self.ui.theme.brand
                } else {
                    muted_color
                };

                let name = if task.name.len() > (inner.width as usize).saturating_sub(4) {
                    format!(
                        "{}..",
                        &task.name[..(inner.width as usize).saturating_sub(7)]
                    )
                } else {
                    task.name.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(icon_color)),
                    Span::styled(
                        name,
                        if is_selected {
                            Style::default()
                                .fg(self.ui.theme.brand)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(text_color)
                        },
                    ),
                ]));
            }

            if self.workspaces.len() > 5 {
                lines.push(Line::from(vec![Span::styled(
                    format!("  +{} more", self.workspaces.len() - 5),
                    Style::default().fg(muted_color),
                )]));
            }
            lines.push(Line::raw(""));
        }

        // Task Summary (if applicable)
        if let Some(task) = self.selected_task_record() {
            lines.push(Line::from(vec![Span::styled(
                "Task",
                Style::default().fg(text_color).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", task.state.user_label()),
                    Style::default().bg(self.ui.theme.brand).fg(Color::Black),
                ),
                Span::styled(
                    format!(" {}", task.pipeline_phase.as_deref().unwrap_or("")),
                    Style::default().fg(muted_color),
                ),
            ]));

            // Use unified trust data parser
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&task.metadata) {
                let trust = UnifiedTrustData::from_metadata(&metadata);

                // Merge status
                if trust.merge_readiness.is_some() || trust.review_summary.is_some() {
                    let (text, color) = if trust.is_merge_ready() {
                        ("READY", self.ui.theme.state.success)
                    } else {
                        ("BLOCKED", self.ui.theme.state.error)
                    };
                    lines.push(Line::from(vec![
                        Span::styled(" Merge ", Style::default().fg(muted_color)),
                        Span::styled(
                            text,
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    // Show blocking count if blocked
                    if !trust.is_merge_ready() && trust.blocking_count() > 0 {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {} blocker(s)", trust.blocking_count()),
                            Style::default().fg(Color::Rgb(220, 100, 100)),
                        )]));
                    }
                }

                // QA status
                if trust.qa_iteration() > 0 {
                    let qa_status = trust.qa_status.as_ref().unwrap();
                    lines.push(Line::from(vec![
                        Span::styled(" QA ", Style::default().fg(muted_color)),
                        Span::styled(
                            format!("{}/{}", qa_status.iteration, qa_status.max_retries),
                            Style::default().fg(text_color),
                        ),
                    ]));
                }

                if trust.needs_escalation() {
                    lines.push(Line::from(vec![Span::styled(
                        " ⚠ ESCALATED",
                        Style::default()
                            .fg(Color::Rgb(220, 100, 100))
                            .add_modifier(Modifier::BOLD),
                    )]));
                }
            }
        } else if !self.git_changes.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Changes",
                Style::default().fg(text_color).add_modifier(Modifier::BOLD),
            )]));
            for change in self.git_changes.iter().take(3) {
                let file_name = std::path::Path::new(&change.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&change.path);
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(file_name, Style::default().fg(muted_color)),
                    if change.added > 0 {
                        Span::styled(
                            format!(" +{}", change.added),
                            Style::default().fg(Color::Rgb(80, 200, 120)),
                        )
                    } else {
                        Span::styled("".to_string(), Style::default())
                    },
                    if change.removed > 0 {
                        Span::styled(
                            format!(" -{}", change.removed),
                            Style::default().fg(Color::Rgb(220, 100, 100)),
                        )
                    } else {
                        Span::styled("".to_string(), Style::default())
                    },
                ]));
            }
        }

        let _ = self.workspaces.len();

        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, inner);
    }
}
