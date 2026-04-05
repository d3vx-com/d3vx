//! Chat text editing, scrolling, history navigation, and send handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::slash_commands;
use crate::app::{App, AppMode};

impl App {
    /// Handle text editing, scrolling, history, and send keys within chat input mode.
    /// Called as a fallthrough from `handle_input_key` for non-control-combo keys.
    pub(crate) fn handle_input_navigation_key(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            // Send message or Handle Multiline
            (KeyCode::Enter, _) => {
                if self.accept_selected_mention()? {
                    return Ok(());
                }
                let input = self.ui.input_buffer.clone();
                self.ui.show_welcome = false; // Dismiss welcome on enter
                self.ui.input_history.push(input.clone());
                self.ui.input_buffer.clear();
                self.ui.cursor_position = 0;
                self.ui.history_index = self.ui.input_history.len();
                self.ui.history_prefix = None;
                self.clear_mention_picker();

                if let Some(tx) = &self.event_tx {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(crate::event::Event::SendMessage(input)).await;
                    });
                }
            }

            // Tab Completion
            (KeyCode::Tab, KeyModifiers::NONE) => {
                if self.select_next_mention(false) {
                    return Ok(());
                }
                self.handle_tab_completion(false)?;
            }
            (KeyCode::BackTab, _) | (KeyCode::Tab, KeyModifiers::SHIFT) => {
                if self.select_next_mention(true) {
                    return Ok(());
                }
                self.handle_tab_completion(true)?;
            }

            // Handle ESC based on context
            (KeyCode::Esc, _) => {
                let now = std::time::Instant::now();
                let duration = now.duration_since(self.ui.last_escape_time);
                self.ui.last_escape_time = now;

                if duration < std::time::Duration::from_millis(500) {
                    self.ui.escape_count += 1;
                } else {
                    self.ui.escape_count = 1;
                }

                // First ESC: dismiss welcome
                if self.ui.show_welcome {
                    self.ui.show_welcome = false;
                    self.ui.escape_count = 0;
                }
                // Double ESC: Undo Picker
                else if self.ui.escape_count >= 2 {
                    let _ = slash_commands::try_execute_slash_command(self, "/undo");
                    self.ui.escape_count = 0;
                }
                // Single ESC when thinking/streaming: stop conversation
                else if self.session.thinking.is_thinking
                    || !self.agents.streaming_message.is_empty()
                {
                    self.stop_conversation()?;
                }
            }

            // Quick Help
            (KeyCode::Char('?'), mods) if mods.is_empty() && self.ui.input_buffer.is_empty() => {
                self.ui.mode = AppMode::Help;
            }

            // Input navigation
            (KeyCode::Left, _) => {
                if self.ui.cursor_position > 0 {
                    self.ui.cursor_position -= 1;
                }
                self.refresh_mention_picker()?;
            }
            (KeyCode::Right, _) => {
                if self.ui.cursor_position < self.ui.input_buffer.len() {
                    self.ui.cursor_position += 1;
                }
                self.refresh_mention_picker()?;
            }

            // Input editing
            (KeyCode::Backspace, _) => {
                if self.ui.cursor_position > 0 {
                    self.ui.input_buffer.remove(self.ui.cursor_position - 1);
                    self.ui.cursor_position -= 1;
                }
                self.refresh_mention_picker()?;
            }
            (KeyCode::Delete, _) => {
                if self.ui.cursor_position < self.ui.input_buffer.len() {
                    self.ui.input_buffer.remove(self.ui.cursor_position);
                }
                self.refresh_mention_picker()?;
            }
            (KeyCode::Char(c), mods)
                if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::ALT) =>
            {
                // Dismiss welcome on typing
                self.ui.show_welcome = false;
                self.ui.input_buffer.insert(self.ui.cursor_position, c);
                self.ui.cursor_position += 1;
                self.ui.history_prefix = None; // Reset search on typing
                self.refresh_mention_picker()?;
            }

            // History Navigation with Prefix Search
            (KeyCode::Up, KeyModifiers::NONE) => {
                if self.select_prev_mention() {
                    return Ok(());
                }
                if self.ui.history_prefix.is_none() && !self.ui.input_buffer.is_empty() {
                    self.ui.history_prefix = Some(self.ui.input_buffer.clone());
                    self.ui.history_index = self.ui.input_history.len();
                }

                if let Some(ref prefix) = self.ui.history_prefix {
                    // Search upwards for matching prefix
                    let mut idx = self.ui.history_index;
                    while idx > 0 {
                        idx -= 1;
                        if self.ui.input_history[idx].starts_with(prefix) {
                            self.ui.history_index = idx;
                            self.ui.input_buffer = self.ui.input_history[idx].clone();
                            self.ui.cursor_position = self.ui.input_buffer.len();
                            self.refresh_mention_picker()?;
                            return Ok(());
                        }
                    }
                } else if !self.ui.input_history.is_empty() && self.ui.history_index > 0 {
                    self.ui.history_index -= 1;
                    self.ui.input_buffer = self.ui.input_history[self.ui.history_index].clone();
                    self.ui.cursor_position = self.ui.input_buffer.len();
                    self.refresh_mention_picker()?;
                }
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                if self.select_next_mention(false) {
                    return Ok(());
                }
                if let Some(ref prefix) = self.ui.history_prefix {
                    // Search downwards for matching prefix
                    let mut idx = self.ui.history_index;
                    while idx + 1 < self.ui.input_history.len() {
                        idx += 1;
                        if self.ui.input_history[idx].starts_with(prefix) {
                            self.ui.history_index = idx;
                            self.ui.input_buffer = self.ui.input_history[idx].clone();
                            self.ui.cursor_position = self.ui.input_buffer.len();
                            self.refresh_mention_picker()?;
                            return Ok(());
                        }
                    }
                    // Reset to prefix if no more matches
                    self.ui.history_index = self.ui.input_history.len();
                    self.ui.input_buffer = prefix.clone();
                    self.ui.cursor_position = self.ui.input_buffer.len();
                    self.refresh_mention_picker()?;
                } else if !self.ui.input_history.is_empty()
                    && self.ui.history_index < self.ui.input_history.len() - 1
                {
                    self.ui.history_index += 1;
                    self.ui.input_buffer = self.ui.input_history[self.ui.history_index].clone();
                    self.ui.cursor_position = self.ui.input_buffer.len();
                    self.refresh_mention_picker()?;
                } else if self.ui.history_index == self.ui.input_history.len() - 1 {
                    self.ui.history_index += 1;
                    self.ui.input_buffer.clear();
                    self.ui.cursor_position = 0;
                    self.clear_mention_picker();
                }
            }
            (KeyCode::Up, KeyModifiers::SHIFT) | (KeyCode::PageUp, _) => {
                self.ui.scroll_offset = self
                    .ui
                    .scroll_offset
                    .saturating_add(5)
                    .min(self.ui.max_scroll.get());
            }
            (KeyCode::Down, KeyModifiers::SHIFT) | (KeyCode::PageDown, _) => {
                self.ui.scroll_offset = self.ui.scroll_offset.saturating_sub(5);
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.ui.scroll_offset = self
                    .ui
                    .scroll_offset
                    .saturating_add(1)
                    .min(self.ui.max_scroll.get());
            }
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                self.ui.scroll_offset = self.ui.scroll_offset.saturating_sub(1);
            }

            // Scroll to top/bottom
            (KeyCode::Home, _) => {
                self.ui.scroll_offset = self.ui.max_scroll.get();
            }
            (KeyCode::End, _) => {
                self.ui.scroll_offset = 0;
            }

            _ => {}
        }
        Ok(())
    }
}
