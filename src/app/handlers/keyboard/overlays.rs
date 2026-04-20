//! Overlay keyboard handling (help, diff view, session picker)

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use tracing::error;

use crate::app::App;

impl App {
    pub(crate) fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                // Return to whichever mode the user was in before `?` /
                // `/help`. Without this, opening Help from Board silently
                // dropped them into Chat on dismiss.
                self.ui.exit_overlay_mode();
                self.ui.help_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.ui.help_scroll = self.ui.help_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.ui.help_scroll = self.ui.help_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.ui.help_scroll = self.ui.help_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.ui.help_scroll = self.ui.help_scroll.saturating_add(10);
            }
            // Ignore other keys to prevent closing help accidentally while trying to scroll
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_diff_view_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.exit_overlay_mode();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut diff) = self.diff_view {
                    diff.scroll_up(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut diff) = self.diff_view {
                    let height = 20; // Approximate viewport height
                    diff.scroll_down(1, height);
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut diff) = self.diff_view {
                    diff.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut diff) = self.diff_view {
                    let height = 20;
                    diff.scroll_down(10, height);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_session_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.exit_overlay_mode();
                self.session_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut picker) = self.session_picker {
                    picker.select_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut picker) = self.session_picker {
                    picker.select_down();
                }
            }
            KeyCode::Enter => {
                if let Some(ref picker) = self.session_picker {
                    if let Some(session_id) = picker.selected_id() {
                        // Use block_in_place to run the async resume_session
                        let session_id = session_id.clone();
                        let result = tokio::task::block_in_place(|| {
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(self.resume_session(&session_id))
                        });

                        if let Err(e) = result {
                            error!(?e, "Failed to resume session");
                            self.add_system_message(&format!("Error: {}", e));
                        }
                    }
                }
                self.ui.exit_overlay_mode();
                self.session_picker = None;
            }
            _ => {}
        }
        Ok(())
    }
}
