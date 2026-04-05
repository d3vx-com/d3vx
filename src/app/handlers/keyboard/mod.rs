//! Keyboard Event Handling

mod board;
mod chat_input;
mod chat_navigation;
mod command_palette;
mod list;
mod mention;
mod model_picker;
mod overlays;
mod undo_picker;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, AppMode};

impl App {
    /// Handle a key event
    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        // Only handle key press events (ignore release)
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // Handle model picker
        if self.ui.show_model_picker {
            self.handle_model_picker_key(key).await?;
            return Ok(());
        }

        // Handle permission request mode (Unified Approval)
        if self.session.permission_request.is_some() {
            match key.code {
                KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Enter => {
                    self.respond_approval(crate::ipc::ApprovalDecision::Approve)?;
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(ref req) = self.session.permission_request {
                        if let Some(ref diff_content) = req.diff {
                            let file_path = req
                                .resource
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string());
                            self.diff_view =
                                Some(crate::ui::widgets::DiffView::new(&file_path, diff_content));
                            self.ui.mode = AppMode::DiffPreview;
                        }
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Esc => {
                    self.respond_approval(crate::ipc::ApprovalDecision::Deny)?;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    self.respond_approval(crate::ipc::ApprovalDecision::ApproveAll)?;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle command palette mode
        if self.ui.mode == AppMode::CommandPalette {
            self.handle_command_palette_key(key)?;
            return Ok(());
        }

        // Handle diff preview mode
        if self.ui.mode == AppMode::DiffPreview {
            self.handle_diff_view_key(key)?;
            return Ok(());
        }

        if self.ui.mode == AppMode::Board {
            self.handle_board_key(key)?;
            return Ok(());
        }

        if self.ui.mode == AppMode::List {
            self.handle_list_key(key)?;
            return Ok(());
        }

        // Handle undo picker mode
        if self.ui.mode == AppMode::UndoPicker {
            self.handle_undo_picker_key(key)?;
            return Ok(());
        }

        // Handle session picker mode
        if self.ui.mode == AppMode::SessionPicker {
            self.handle_session_picker_key(key)?;
            return Ok(());
        }

        // Handle help mode
        if self.ui.mode == AppMode::Help {
            self.handle_help_key(key)?;
            return Ok(());
        }

        // Handle input mode
        self.handle_input_key(key)?;

        Ok(())
    }
}
