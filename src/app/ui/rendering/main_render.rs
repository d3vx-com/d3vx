//! Main render entry point and toast notifications

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::state::NotificationType;
use crate::app::{App, AppMode};
use crate::ui::symbols::STATUS;

/// Helper to create a centered rect (only used within this module)
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl App {
    /// Main render entry point
    pub fn render(&mut self, f: &mut Frame) {
        let size = f.area();

        // Check for specific modes that override the main layout
        match self.ui.mode {
            AppMode::DiffPreview => {
                if let Some(ref diff) = self.diff_view {
                    diff.render(f, size, &self.ui.theme);
                    return;
                }
            }
            AppMode::UndoPicker => {
                if let Some(ref picker) = self.undo_picker {
                    picker.render(f, size, &self.ui.theme);
                    return;
                }
            }
            AppMode::SessionPicker => {
                if let Some(ref picker) = self.session_picker {
                    picker.render(f, size, &self.ui.theme);
                    return;
                }
            }
            AppMode::Board => {
                let _ = self.refresh_task_views();
                if self.ui.right_sidebar_visible {
                    // Continues to main layout split below
                } else {
                    self.board.render(f, size);
                    return;
                }
            }
            AppMode::List => {
                let _ = self.refresh_task_views();
                if self.ui.right_sidebar_visible {
                    // Continues to main layout split below
                } else {
                    self.render_task_list(f, size);
                    return;
                }
            }
            _ => {}
        }

        // Root Layout: Content only (status bar moved to activity panel)
        let content_area = size;

        // Main Layout: [Main Area | Activity Panel / Sidebar]
        let (main_area, right_area) =
            if self.ui.mode == AppMode::Board || self.ui.mode == AppMode::List {
                if self.ui.right_sidebar_visible {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Min(0), Constraint::Percentage(25)])
                        .split(content_area);
                    (chunks[0], Some(chunks[1]))
                } else {
                    (content_area, None)
                }
            } else if self.ui.right_sidebar_visible {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(content_area);
                (chunks[0], Some(chunks[1]))
            } else {
                (content_area, None)
            };

        let activity_area = right_area;

        // Simple vertical split: chat (flex) + input (fixed)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3), // Input area + focus mode chips
            ])
            .split(main_area);

        let chat_area = main_chunks[0];
        let input_area = main_chunks[1];

        // Store rects for mouse hit-testing
        self.layout.last_left_sidebar_rect = Rect::default();
        self.layout.last_right_sidebar_rect = activity_area.unwrap_or(Rect::default());
        self.layout.last_input_rect = input_area;
        self.layout.last_chat_rect = chat_area;

        // Render welcome or messages or board
        if self.ui.mode == AppMode::Board {
            self.board.render(f, chat_area);
        } else if self.ui.mode == AppMode::List {
            self.render_task_list(f, chat_area);
        } else if self.ui.show_welcome && self.session.messages.is_empty() {
            self.render_welcome(f, chat_area);
        } else {
            // Add padding around chat area
            let chat_inner_area = Rect {
                x: chat_area.x + 2,
                y: chat_area.y + 1,
                width: chat_area.width.saturating_sub(4),
                height: chat_area.height.saturating_sub(2),
            };

            let messages = self.render_messages(chat_inner_area);
            f.render_widget(messages, chat_inner_area);
        }

        // Render activity panel with ratatui border separator
        if let Some(activity_rect) = activity_area {
            self.layout.last_activity_rect = Some(activity_rect);

            // Draw a left border on the sidebar for visual separation
            let border_block = Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 75)))
                .border_type(BorderType::Plain);
            let sidebar_inner = border_block.inner(activity_rect);
            f.render_widget(border_block, activity_rect);
            self.render_activity_panel(f, sidebar_inner);
        }

        // Render input
        self.render_input(f, input_area);

        if !self.ui.mention_suggestions.is_empty() {
            self.render_mention_picker(f, size, input_area);
        }

        // Render unified sidebar for Board/List modes only
        if (self.ui.mode == AppMode::Board || self.ui.mode == AppMode::List)
            && self.ui.right_sidebar_visible
        {
            if let Some(sidebar_area) = right_area {
                f.render_widget(Clear, sidebar_area);
                self.render_sidebar(f, sidebar_area);
            }
        }

        // Render command palette overlay
        if self.ui.mode == AppMode::CommandPalette {
            self.render_command_palette(f, size);
        }

        // Render permission request overlay
        if let Some(ref req) = self.session.permission_request {
            let area = centered_rect(60, 30, size);
            f.render_widget(Clear, area);
            self.render_permission_request(f, area, req);
        }

        // Render Help overlay
        if self.ui.mode == AppMode::Help {
            f.render_widget(
                crate::ui::widgets::HelpModal::new(self.ui.theme.clone(), self.ui.help_scroll),
                size,
            );
        }

        // Render Model Picker overlay
        if self.ui.show_model_picker {
            let registry = self.registry.blocking_read();
            let models = registry.list_models();
            crate::ui::widgets::model_picker::ModelPicker::render(
                f,
                size,
                &self.ui,
                &models,
                &self.ui.theme,
            );
        }

        // Render notifications (toasts)
        self.render_notifications(f, size);
    }

    /// Render toast notifications
    fn render_notifications(&self, f: &mut Frame, area: Rect) {
        if self.notifications.is_empty() {
            return;
        }

        let notification_width = 40;
        let notification_height = 3;

        for (idx, notification) in self.notifications.iter().enumerate() {
            let y_offset = (idx as u16) * (notification_height + 1);
            let toast_area = Rect {
                x: area.width.saturating_sub(notification_width + 2),
                y: 1 + y_offset,
                width: notification_width,
                height: notification_height,
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(match notification.notification_type {
                    NotificationType::Success => self.ui.theme.state.success,
                    NotificationType::Error => self.ui.theme.state.error,
                    NotificationType::Info => self.ui.theme.brand_secondary,
                }))
                .title(match notification.notification_type {
                    NotificationType::Success => {
                        format!(" {} DONE ", STATUS.success)
                    }
                    NotificationType::Error => {
                        format!(" {} ERROR ", STATUS.error)
                    }
                    NotificationType::Info => {
                        format!(" {} INFO ", STATUS.info)
                    }
                });

            f.render_widget(Clear, toast_area);
            f.render_widget(
                Paragraph::new(notification.message.clone()).block(block),
                toast_area,
            );
        }
    }
}
