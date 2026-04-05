//! Session Picker Widget
//!
//! Renders a list of previous sessions that can be resumed,
//! allowing users to restore past conversations.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};

use crate::store::session::Session;
use crate::ui::theme::Theme;

/// Session picker state
#[derive(Debug, Clone, Default)]
pub struct SessionPicker {
    /// List of sessions
    pub sessions: Vec<Session>,
    /// Currently selected index
    pub selected: usize,
}

impl SessionPicker {
    /// Create session picker from a list of sessions
    pub fn new(sessions: Vec<Session>) -> Self {
        Self {
            sessions,
            selected: 0,
        }
    }

    /// Move selection up
    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_down(&mut self) {
        if !self.sessions.is_empty() && self.selected + 1 < self.sessions.len() {
            self.selected += 1;
        }
    }

    /// Get the selected session ID
    pub fn selected_id(&self) -> Option<String> {
        self.sessions.get(self.selected).map(|s| s.id.clone())
    }

    /// Render the session picker overlay
    pub fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
        // Calculate popup dimensions
        let popup_height = std::cmp::min(20, area.height.saturating_sub(4));
        let popup_width = std::cmp::min(100, area.width.saturating_sub(4));

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
            .sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let is_selected = i == self.selected;
                let style = if is_selected {
                    Style::default()
                        .fg(theme.brand)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Extract first message if summary is missing
                let description = if let Some(summary) = &session.summary {
                    if summary.len() > 60 {
                        format!("{}...", &summary.chars().take(57).collect::<String>())
                    } else {
                        summary.clone()
                    }
                } else {
                    // Try to parse first user message
                    let messages: Result<Vec<serde_json::Value>, _> =
                        serde_json::from_str(&session.messages);
                    match messages {
                        Ok(msgs) => {
                            let first_user = msgs
                                .iter()
                                .find(|m| m["role"] == "user")
                                .and_then(|m| m["content"].as_str());

                            if let Some(content) = first_user {
                                let mut clean_content = content.replace('\n', " ");
                                if clean_content.len() > 60 {
                                    clean_content = format!(
                                        "{}...",
                                        clean_content.chars().take(57).collect::<String>()
                                    );
                                }
                                format!("\"{}\"", clean_content)
                            } else {
                                "No messages".to_string()
                            }
                        }
                        Err(_) => "Session data error".to_string(),
                    }
                };

                let provider_model = format!("{}/{}", session.provider, session.model);
                let timestamp = &session.created_at; // ISO 8601 string
                let display_id = if session.id.len() > 12 {
                    format!("{}...", &session.id[0..9])
                } else {
                    session.id.clone()
                };

                let content = Line::from(vec![
                    Span::styled(
                        format!("{:<13} ", display_id),
                        Style::default().fg(theme.brand),
                    ),
                    Span::styled(
                        timestamp.chars().take(16).collect::<String>(),
                        Style::default().fg(theme.ui.text_dim),
                    ),
                    Span::styled(" │ ", Style::default().fg(theme.ui.border)),
                    Span::styled(
                        format!("{:<20} ", provider_model),
                        Style::default().fg(theme.ui.text_muted),
                    ),
                    Span::styled(" │ ", Style::default().fg(theme.ui.border)),
                    Span::styled(description, style),
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
                .title(" Resume Session (↑↓ to navigate, Enter to select, Esc to cancel) ")
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
