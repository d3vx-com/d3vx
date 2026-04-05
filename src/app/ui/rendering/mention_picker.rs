//! File mention picker and permission request overlay rendering

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ipc::PermissionRequest;

impl App {
    pub(super) fn render_mention_picker(&self, f: &mut Frame, area: Rect, input_area: Rect) {
        let popup_width = input_area.width.min(60).max(24);
        let visible_count = self.ui.mention_suggestions.len().min(6);
        let popup_height = (visible_count as u16)
            .saturating_add(2)
            .min(area.height.max(3));
        let x = input_area.x;
        let y = input_area.y.saturating_sub(popup_height);
        let popup_area = Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(Clear, popup_area);

        let items: Vec<ListItem> = self
            .ui
            .mention_suggestions
            .iter()
            .take(visible_count)
            .enumerate()
            .map(|(idx, suggestion)| {
                let selected = idx
                    == self
                        .ui
                        .mention_selected
                        .min(visible_count.saturating_sub(1));
                let content = Line::from(vec![
                    Span::styled("@", Style::default().fg(self.ui.theme.brand_secondary)),
                    Span::styled(
                        suggestion.clone(),
                        if selected {
                            Style::default()
                                .fg(self.ui.theme.brand)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(self.ui.theme.ui.text)
                        },
                    ),
                ]);

                ListItem::new(content).style(if selected {
                    Style::default().bg(self.ui.theme.ui.border)
                } else {
                    Style::default()
                })
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" File Mentions ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.ui.theme.brand_secondary)),
        );

        f.render_widget(list, popup_area);
    }

    /// Render permission request (Unified Approval)
    pub fn render_permission_request(&self, f: &mut Frame, area: Rect, req: &PermissionRequest) {
        let block = Block::default()
            .title(Span::styled(
                " COMMAND APPROVAL ",
                Style::default()
                    .fg(self.ui.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.ui.theme.brand));

        let _inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let mut lines = Vec::new();

        // Tool Name
        if let Some(ref tool_name) = req.tool_name {
            lines.push(Line::from(vec![
                Span::styled("Tool: ", Style::default().fg(self.ui.theme.ui.text_muted)),
                Span::styled(
                    tool_name,
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Message
        lines.push(Line::from(vec![
            Span::styled("Action: ", Style::default().fg(self.ui.theme.ui.text_muted)),
            Span::styled(&req.message, Style::default().fg(self.ui.theme.ui.text)),
        ]));

        lines.push(Line::raw(""));

        // Diff Preview Hint (if available)
        if req.diff.is_some() {
            lines.push(Line::from(vec![Span::styled(
                " \u{2139} Diff preview available (Ctrl+D) ",
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::ITALIC),
            )]));
            lines.push(Line::raw(""));
        } else if let Some(ref tool_name) = req.tool_name {
            if tool_name.to_lowercase().contains("edit")
                || tool_name.to_lowercase().contains("write")
            {
                lines.push(Line::from(vec![Span::styled(
                    " \u{2139} This tool modifies files but no diff is generated yet. ",
                    Style::default().fg(self.ui.theme.ui.text_dim),
                )]));
                lines.push(Line::raw(""));
            }
        }

        // Controls
        lines.push(Line::from(vec![
            Span::styled(
                " [A] Approve ",
                Style::default()
                    .bg(self.ui.theme.state.success)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                " [D] Deny ",
                Style::default()
                    .bg(self.ui.theme.state.error)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                " [V] Always Trust ",
                Style::default()
                    .bg(self.ui.theme.brand)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }
}
