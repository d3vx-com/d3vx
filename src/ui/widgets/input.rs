//! Input Widget
//!
//! A text input field with cursor support.

use crate::ui::theme::Theme;
use ratatui::{
    style::Style,
    text::Span,
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Input widget state
pub struct InputState {
    /// Current input buffer
    pub buffer: String,
    /// Cursor position (character index)
    pub cursor: usize,
    /// Autocomplete suggestions
    pub suggestions: Vec<String>,
    /// Selected suggestion index
    pub suggestion_index: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            suggestions: Vec::new(),
            suggestion_index: 0,
        }
    }
}

impl InputState {
    /// Create a new input state
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a character at cursor position
    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the start of the previous character
            let prev_char_start = self.buffer[..self.cursor]
                .char_indices()
                .rev()
                .next()
                .map(|(i, _)| i)
                .unwrap_or(0);

            self.buffer.remove(prev_char_start);
            self.cursor = prev_char_start;
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.buffer[..self.cursor]
                .char_indices()
                .rev()
                .next()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.buffer[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.buffer.len());
            self.cursor = next;
        }
    }

    /// Move cursor to start
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Clear input
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Take the buffer content, clearing it
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        self.suggestions.clear();
        self.suggestion_index = 0;
        std::mem::take(&mut self.buffer)
    }

    /// Complete current input with selected suggestion
    pub fn complete(&mut self) {
        if !self.suggestions.is_empty() {
            self.buffer = self.suggestions[self.suggestion_index].clone();
            self.cursor = self.buffer.len();
            self.suggestions.clear();
            self.suggestion_index = 0;
        }
    }

    /// Next suggestion
    pub fn next_suggestion(&mut self) {
        if !self.suggestions.is_empty() {
            self.suggestion_index = (self.suggestion_index + 1) % self.suggestions.len();
        }
    }
}

/// Input widget
pub struct InputWidget<'a> {
    state: &'a InputState,
    theme: Theme,
    title: &'a str,
    placeholder: Option<&'a str>,
    focused: bool,
}

impl<'a> InputWidget<'a> {
    /// Create a new input widget
    pub fn new(state: &'a InputState) -> Self {
        Self {
            state,
            theme: Theme::dark(),
            title: " Input ",
            placeholder: None,
            focused: true,
        }
    }

    /// Set theme
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Set title
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Set placeholder text
    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = Some(text);
        self
    }

    /// Set focused state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl<'a> Widget for InputWidget<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let border_color = if self.focused {
            self.theme.ui.border
        } else {
            self.theme.ui.border
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.title);

        let inner = block.inner(area);
        block.render(area, buf);

        // Build display content
        let display_text = if self.state.buffer.is_empty() {
            self.placeholder.unwrap_or("").to_string()
        } else {
            self.state.buffer.clone()
        };

        let style = if self.state.buffer.is_empty() && self.placeholder.is_some() {
            Style::default().fg(self.theme.ui.text_dim)
        } else {
            Style::default().fg(self.theme.ui.text)
        };

        // Render text with cursor
        let text = Span::styled(&display_text, style);
        let paragraph = Paragraph::new(text);
        paragraph.render(inner, buf);

        // Note: In ratatui, cursor positioning needs to be handled separately
        // via terminal.set_cursor() in the main loop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_insert() {
        let mut state = InputState::new();
        state.insert('a');
        state.insert('b');
        state.insert('c');
        assert_eq!(state.buffer, "abc");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn test_input_backspace() {
        let mut state = InputState::new();
        state.insert('a');
        state.insert('b');
        state.backspace();
        assert_eq!(state.buffer, "a");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn test_input_navigation() {
        let mut state = InputState::new();
        state.insert('a');
        state.insert('b');
        state.insert('c');
        state.move_left();
        assert_eq!(state.cursor, 2);
        state.move_right();
        assert_eq!(state.cursor, 3);
    }
}
