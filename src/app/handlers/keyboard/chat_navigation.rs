//! Chat text editing, scrolling, history navigation, and send handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::slash_commands;
use crate::app::{App, AppMode};

impl App {
    /// Handle pasted text — bulk-insert into input buffer at cursor position.
    ///
    /// For large pastes (>3 lines or >150 chars), shows a summary like
    /// `[Pasted ~42 lines, 1.2K chars]` in the input buffer instead of
    /// dumping all the text. The actual content is stored separately and
    /// expanded on send.
    pub(crate) fn handle_paste(&mut self, text: String) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        // Normalize line endings
        let text = text.replace('\r', "");

        let line_count = text.lines().count();
        let char_count = text.len();

        // Threshold: 3+ lines or 150+ characters triggers summary view
        if line_count >= 3 || char_count > 150 {
            let preview = format_paste_preview(line_count, char_count);
            let pos = self.ui.cursor_position;

            // Store actual content for later expansion
            self.ui.pending_paste = Some(text);
            self.ui.paste_preview = Some(preview.clone());

            // Insert preview text into input buffer
            self.ui.input_buffer.insert_str(pos, &preview);
            self.ui.cursor_position = pos + preview.len();
        } else {
            // Small paste: insert directly
            let pos = self.ui.cursor_position;
            self.ui.input_buffer.insert_str(pos, &text);
            self.ui.cursor_position = pos + text.len();
        }

        self.ui.show_welcome = false;
        self.refresh_mention_picker()?;
        Ok(())
    }

    /// Handle text editing, scrolling, history, and send keys within chat input mode.
    /// Called as a fallthrough from `handle_input_key` for non-control-combo keys.
    pub(crate) fn handle_input_navigation_key(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            // Send message or Handle Multiline
            (KeyCode::Enter, _) => {
                if self.accept_selected_mention()? {
                    return Ok(());
                }
                // Expand paste preview into actual content before sending
                let input = self.expand_paste_content();
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

                // First ESC: close drawer if open
                if self.ui.drawer_height != crate::app::state::DrawerHeight::Closed {
                    self.save_scroll_anchor();
                    self.ui.drawer_height = crate::app::state::DrawerHeight::Closed;
                    self.ui.escape_count = 0;
                }
                // Second: dismiss welcome
                else if self.ui.show_welcome {
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

    /// Expand paste preview into actual content.
    ///
    /// If there's a pending paste, replaces the `[Pasted ~N lines...]` marker
    /// in the input buffer with the actual pasted content. Clears the pending
    /// state after expansion.
    fn expand_paste_content(&mut self) -> String {
        let input = self.ui.input_buffer.clone();

        if let (Some(actual), Some(preview)) =
            (self.ui.pending_paste.take(), self.ui.paste_preview.take())
        {
            // Only expand if the preview text is still in the buffer.
            // If the user deleted or edited it away, use the buffer as-is.
            if input.contains(&preview) {
                input.replace(&preview, &actual)
            } else {
                input
            }
        } else {
            input
        }
    }
}

/// Format a paste preview string for display in the input buffer.
fn format_paste_preview(line_count: usize, char_count: usize) -> String {
    let chars_display = if char_count >= 1000 {
        format!("{:.1}K chars", char_count as f64 / 1000.0)
    } else {
        format!("{} chars", char_count)
    };
    format!("[Pasted ~{} lines, {}]", line_count, chars_display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_preview_small() {
        assert_eq!(format_paste_preview(5, 200), "[Pasted ~5 lines, 200 chars]");
    }

    #[test]
    fn test_format_preview_large() {
        assert_eq!(
            format_paste_preview(120, 8000),
            "[Pasted ~120 lines, 8.0K chars]"
        );
    }

    #[test]
    fn test_format_preview_exact_1k() {
        assert_eq!(
            format_paste_preview(10, 1000),
            "[Pasted ~10 lines, 1.0K chars]"
        );
    }

    #[test]
    fn test_format_preview_below_1k() {
        assert_eq!(format_paste_preview(3, 999), "[Pasted ~3 lines, 999 chars]");
    }

    #[test]
    fn test_expand_does_not_replace_deleted_preview() {
        // If the user deletes the [Pasted...] text from the buffer,
        // expand should return the buffer as-is, not try to replace.
        let preview = format_paste_preview(5, 200);
        let actual = "actual pasted content here".to_string();

        // Simulate: buffer no longer contains the preview
        let buffer = "something else entirely".to_string();
        assert!(!buffer.contains(&preview));

        // Manual expansion logic (mirrors expand_paste_content)
        let result = if buffer.contains(&preview) {
            buffer.replace(&preview, &actual)
        } else {
            buffer
        };
        assert_eq!(result, "something else entirely");
    }

    #[test]
    fn test_expand_replaces_when_preview_present() {
        let preview = format_paste_preview(5, 200);
        let actual = "line1\nline2\nline3\nline4\nline5\n".to_string();

        let buffer = format!("explain this: {}", preview);
        let result = if buffer.contains(&preview) {
            buffer.replace(&preview, &actual)
        } else {
            buffer
        };
        assert_eq!(result, format!("explain this: {}", actual));
    }
}
