//! Command palette overlay rendering

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

impl App {
    /// Render the command palette
    pub fn render_command_palette(&self, f: &mut Frame, area: Rect) {
        let popup_width = std::cmp::min(60, area.width);
        let popup_height = std::cmp::min(20, area.height.saturating_sub(4));

        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(Clear, popup_area);

        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(popup_area);

        let search_text = if self.command_palette_filter.is_empty() {
            Span::styled(
                "Search commands...",
                Style::default().fg(self.ui.theme.ui.text_muted),
            )
        } else {
            Span::raw(&self.command_palette_filter)
        };

        let search = Paragraph::new(Line::from(search_text)).block(
            Block::default()
                .title(" Command Palette ")
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.ui.theme.brand)),
        );
        f.render_widget(search, popup_layout[0]);

        let commands = self.get_filtered_commands();
        let items: Vec<ListItem> = commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let is_selected = i == self.command_palette_selected;
                let style = if is_selected {
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let content = Line::from(vec![
                    Span::styled(format!("/{} ", cmd.name), style),
                    Span::styled(
                        if cmd.description.len() > 40 {
                            format!("{}...", &cmd.description[..37])
                        } else {
                            cmd.description.to_string()
                        },
                        Style::default().fg(self.ui.theme.ui.text_muted),
                    ),
                ]);

                ListItem::new(content).style(if is_selected {
                    Style::default().bg(self.ui.theme.ui.border)
                } else {
                    Style::default()
                })
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(Style::default().fg(self.ui.theme.brand)),
        );

        f.render_widget(list, popup_layout[1]);
    }
}
