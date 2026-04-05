//! Text utilities - Unicode handling, truncation, wrapping

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ────────────────────────────────────────────────────────────
// Truncation
// ────────────────────────────────────────────────────────────

/// Truncate a string to fit within a given width (in terminal cells)
pub fn truncate(text: &str, max_width: usize) -> String {
    let width = text.width();
    if width <= max_width {
        return text.to_string();
    }

    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let char_width = ch.width().unwrap_or(0);
        if current_width + char_width + 1 > max_width {
            result.push_str("…");
            break;
        }
        result.push(ch);
        current_width += char_width;
    }

    result
}

/// Truncate a long string by keeping 80% of the start and 20% of the end.
/// This is useful for preserving context in long log files or tool results.
pub fn truncate_80_20(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars || max_chars < 10 {
        return text.to_string();
    }

    let keep_start = (max_chars as f64 * 0.8) as usize;
    let keep_end = max_chars - keep_start;

    let start: String = text.chars().take(keep_start).collect();
    let end: String = text.chars().skip(char_count - keep_end).collect();

    let removed = char_count - keep_start - keep_end;
    format!(
        "{}\n\n[...truncated {} characters...]\n\n{}",
        start, removed, end
    )
}

/// Truncate a path, keeping the filename and as much of the end as possible
pub fn truncate_path(path: &str, max_width: usize) -> String {
    let width = path.width();
    if width <= max_width {
        return path.to_string();
    }

    // Try to keep the last parts of the path (filename + as many parent dirs as possible)
    if let Some(pos) = path.rfind('/') {
        let filename = &path[pos + 1..];
        let filename_width = filename.width();

        if filename_width >= max_width {
            return truncate(filename, max_width);
        }

        // We want to show as much of the end as possible with "…" prefix
        // Find how much we can show from the end
        let available = max_width.saturating_sub(1); // -1 for "…"
        let end_portion = &path[path.len().saturating_sub(available)..];

        // Find the first slash in the end portion to start from a directory boundary
        if let Some(slash_pos) = end_portion.find('/') {
            format!("…{}", &end_portion[slash_pos..])
        } else {
            format!("…/{}", filename)
        }
    } else {
        truncate(path, max_width)
    }
}

/// Truncate a command string, preserving the command name
pub fn truncate_command(cmd: &str, max_width: usize) -> String {
    let width = cmd.width();
    if width <= max_width {
        return cmd.to_string();
    }

    // Find the command name (first word)
    if let Some(space_pos) = cmd.find(' ') {
        let cmd_name = &cmd[..space_pos];
        let args = &cmd[space_pos + 1..];

        let cmd_name_width = cmd_name.width();
        if cmd_name_width >= max_width {
            return truncate(cmd_name, max_width);
        }

        let remaining_width = max_width - cmd_name_width - 4; // -4 for " …"
        let truncated_args = truncate(args, remaining_width);

        format!("{} {}", cmd_name, truncated_args)
    } else {
        truncate(cmd, max_width)
    }
}

// ────────────────────────────────────────────────────────────
// Word Wrap
// ────────────────────────────────────────────────────────────

/// Wrap text to a given width, returning a vector of lines
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        let wrapped = wrap_paragraph(paragraph, width);
        lines.extend(wrapped);
    }

    lines
}

fn wrap_paragraph(text: &str, width: usize) -> Vec<String> {
    if text.width() <= width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.width();

        if current_width == 0 {
            // First word on line
            current_line.push_str(word);
            current_width = word_width;
        } else if current_width + 1 + word_width <= width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

// ────────────────────────────────────────────────────────────
// String Metrics
// ────────────────────────────────────────────────────────────

/// Calculate the display width of a string (accounting for wide chars)
pub fn display_width(s: &str) -> usize {
    s.width()
}

/// Calculate the display width of a single character
pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Count the number of characters (not bytes) in a string
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Get the byte position for a given character position
pub fn char_position_to_byte(text: &str, char_pos: usize) -> usize {
    text.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

/// Get the display position for a given byte position
pub fn byte_position_to_display(text: &str, byte_pos: usize) -> usize {
    let mut display_pos = 0;
    for (i, ch) in text.char_indices() {
        if i >= byte_pos {
            break;
        }
        display_pos += ch.width().unwrap_or(0);
    }
    display_pos
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello", 10), "Hello");
        assert_eq!(truncate("Hello World", 8), "Hello W…");
        assert_eq!(truncate("Hello", 3), "He…");
    }

    #[test]
    fn test_truncate_80_20() {
        let text = "0123456789".repeat(10); // 100 chars
        let truncated = truncate_80_20(&text, 50);
        assert!(truncated.contains("truncated 50 characters"));
        assert!(truncated.starts_with("0123456789012345678901234567890123456789")); // 40 chars (80% of 50)
        assert!(truncated.ends_with("0123456789")); // 10 chars (20% of 50)
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(
            truncate_path("/Users/test/documents/file.txt", 20),
            "…/documents/file.txt"
        );
    }

    #[test]
    fn test_wrap_text() {
        let wrapped = wrap_text("Hello world this is a test", 10);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.width() <= 10);
        }
    }

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("Hello"), 5);
        assert_eq!(display_width("你好"), 4); // Chinese chars are wide
        assert_eq!(display_width("🎉"), 2); // Emoji
    }
}
