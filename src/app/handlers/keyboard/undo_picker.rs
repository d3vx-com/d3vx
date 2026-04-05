//! Undo picker keyboard handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode};

impl App {
    pub(crate) fn handle_undo_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.mode = AppMode::Chat;
                self.undo_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut picker) = self.undo_picker {
                    picker.select_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut picker) = self.undo_picker {
                    picker.select_down();
                }
            }
            KeyCode::Enter => {
                if let Some(ref picker) = self.undo_picker {
                    if let Some(undo_index) = picker.selected_index() {
                        // 1. Revert files changed after this point
                        let reverted = self.session.file_change_log.revert_to(undo_index);

                        // 2. Truncate messages to the selected point
                        self.session.messages.truncate(undo_index + 1);

                        // 3. Clean up changelog entries after this point
                        self.session.file_change_log.truncate(undo_index);

                        self.ui.scroll_offset = 0;

                        // 4. Notify user
                        if reverted.is_empty() {
                            self.add_system_message(&format!(
                                "Restored to message {}",
                                undo_index + 1
                            ));
                        } else {
                            let file_names: Vec<String> = reverted
                                .iter()
                                .map(|p| {
                                    std::path::Path::new(p)
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string()
                                })
                                .collect();
                            self.add_system_message(&format!(
                                "Restored to message {}. Reverted {} file(s): {}",
                                undo_index + 1,
                                reverted.len(),
                                file_names.join(", ")
                            ));
                        }
                    }
                }
                self.ui.mode = AppMode::Chat;
                self.undo_picker = None;
            }
            _ => {}
        }
        Ok(())
    }
}
