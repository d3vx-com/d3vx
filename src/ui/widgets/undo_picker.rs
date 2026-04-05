//! Undo Picker Widget
//!
//! Renders a list of recent messages/actions that can be undone,
//! allowing users to restore to a previous state.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};

use crate::agent::file_change_log::FileChangeLog;
use crate::ipc::Message;
use crate::ui::theme::Theme;

/// Undo item representing a restore point
#[derive(Debug, Clone)]
pub struct UndoItem {
    /// Index in the message history
    pub index: usize,
    /// Brief description of the message/action
    pub description: String,
    /// Timestamp
    pub timestamp: String,
    /// Role of the message sender
    pub role: String,
    /// Files that will be reverted if this undo point is selected
    pub files_affected: Vec<String>,
}

/// Undo picker state
#[derive(Debug, Clone, Default)]
pub struct UndoPicker {
    /// List of undo items
    pub items: Vec<UndoItem>,
    /// Currently selected index
    pub selected: usize,
}

impl UndoPicker {
    /// Create undo picker from messages and file change log
    pub fn from_messages(messages: &[Message], change_log: &FileChangeLog) -> Self {
        let items: Vec<UndoItem> = messages
            .iter()
            .enumerate()
            .rev() // Most recent first
            .map(|(index, msg)| {
                let role = match msg.role {
                    crate::ipc::types::MessageRole::User => "You",
                    crate::ipc::types::MessageRole::Assistant => "Assistant",
                    crate::ipc::types::MessageRole::System => "System",
                    crate::ipc::types::MessageRole::Shell => "Shell",
                };

                // Truncate content for display
                let description = if msg.content.len() > 60 {
                    format!("{}...", &msg.content.chars().take(57).collect::<String>())
                } else {
                    msg.content.chars().take(60).collect()
                };

                let timestamp = msg.timestamp.format("%H:%M:%S").to_string();

                let files_affected = change_log.files_after(index);

                UndoItem {
                    index,
                    description,
                    timestamp,
                    role: role.to_string(),
                    files_affected,
                }
            })
            .collect();

        Self { items, selected: 0 }
    }

    /// Move selection up
    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Get the selected undo index
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.selected).map(|item| item.index)
    }

    /// Render the undo picker overlay
    pub fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
        // Calculate popup dimensions
        let popup_height = std::cmp::min(20, area.height.saturating_sub(4));
        let popup_width = std::cmp::min(80, area.width);

        let popup_area = ratatui::layout::Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        // Clear the area first
        f.render_widget(Clear, popup_area);

        // Build list items
        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == self.selected;
                let style = if is_selected {
                    Style::default()
                        .fg(theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let role_style = match item.role.as_str() {
                    "You" => Style::default().fg(theme.role.user),
                    "Assistant" => Style::default().fg(theme.role.assistant),
                    _ => Style::default().fg(theme.ui.text_muted),
                };

                let file_hint = if item.files_affected.is_empty() {
                    String::new()
                } else {
                    format!(" ({} files)", item.files_affected.len())
                };

                let content = Line::from(vec![
                    Span::styled(
                        format!("{:>8} ", item.timestamp),
                        Style::default().fg(theme.ui.text_dim),
                    ),
                    Span::styled(format!("{:<10} ", item.role), role_style),
                    Span::styled(&item.description, style),
                    if !file_hint.is_empty() {
                        Span::styled(file_hint, Style::default().fg(theme.ui.text_muted))
                    } else {
                        Span::raw("")
                    },
                ]);

                ListItem::new(content).style(if is_selected {
                    Style::default().bg(theme.ui.border)
                } else {
                    Style::default()
                })
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" Undo History (↑↓ to navigate, Enter to select, Esc to cancel) ")
                .title_style(
                    Style::default()
                        .fg(theme.brand)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand)),
        );

        f.render_widget(list, popup_area);
    }
}
