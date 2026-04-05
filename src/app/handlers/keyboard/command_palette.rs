//! Command palette keyboard handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::slash_commands;
use crate::app::{App, AppMode};

impl App {
    pub(crate) fn handle_command_palette_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.ui.mode = AppMode::Chat;
                self.command_palette_filter.clear();
                self.command_palette_selected = 0;
            }
            KeyCode::Up => {
                if self.command_palette_selected > 0 {
                    self.command_palette_selected -= 1;
                }
            }
            KeyCode::Down => {
                let filtered_count = self.get_filtered_commands().len();
                if self.command_palette_selected + 1 < filtered_count {
                    self.command_palette_selected += 1;
                }
            }
            KeyCode::Enter => {
                let commands = self.get_filtered_commands();
                if let Some(cmd) = commands.get(self.command_palette_selected) {
                    let cmd_name = cmd.name.to_string();
                    self.ui.mode = AppMode::Chat;
                    self.command_palette_filter.clear();
                    self.command_palette_selected = 0;
                    // Execute the command
                    let _ =
                        slash_commands::try_execute_slash_command(self, &format!("/{}", cmd_name));
                }
            }
            KeyCode::Backspace => {
                self.command_palette_filter.pop();
                self.command_palette_selected = 0;
            }
            KeyCode::Char(c) => {
                self.command_palette_filter.push(c);
                self.command_palette_selected = 0;
            }
            _ => {}
        }
        Ok(())
    }
}
