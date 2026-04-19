//! Markdown text widget.
//!
//! Public facade over the renderer in
//! [`markdown_renderer`](super::markdown_renderer). Renders a markdown
//! string into a `Vec<Line<'static>>` suitable for a ratatui
//! `Paragraph`, emphasising a minimal, tidy output:
//!
//! - Tables use Unicode box-drawing with shared borders (one contiguous
//!   grid, not disjoint per-column rectangles).
//! - At most one consecutive blank line between blocks.
//! - `[text](url)` renders as `text (url)` with the URL dimmed.
//! - Inline code uses a background colour — no literal backticks in the
//!   visible text.
//! - Blockquotes get a `│ ` prefix.
//!
//! Callers stay on the previous API (`MarkdownText::new(…).theme(…)`),
//! so message views don't change.

use crate::ui::theme::Theme;
use ratatui::text::Line;

use super::markdown_renderer::MarkdownRenderer;

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    /// Target width used when rendering horizontal rules. Lines
    /// themselves are not hard-wrapped here — the ratatui `Paragraph`
    /// that consumes the output does the wrapping.
    pub max_width: usize,
    /// Show the full content of code blocks. When `false`, blocks
    /// longer than `max_code_lines` are truncated with a hint.
    pub verbose: bool,
    /// Cap on visible lines per code block when `verbose` is `false`.
    pub max_code_lines: usize,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            max_width: 80,
            verbose: false,
            max_code_lines: 10,
        }
    }
}

/// A markdown-formatted text block, ready to be turned into a `Vec<Line>`.
pub struct MarkdownText {
    content: String,
    config: MarkdownConfig,
    theme: Theme,
}

impl MarkdownText {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            config: MarkdownConfig::default(),
            theme: Theme::dark(),
        }
    }

    pub fn config(mut self, config: MarkdownConfig) -> Self {
        self.config = config;
        self
    }

    pub fn verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Render the markdown to a vec of styled lines.
    pub fn render(&self) -> Vec<Line<'static>> {
        MarkdownRenderer::new(&self.theme, &self.config).render(&self.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_all(md: &str) -> Vec<String> {
        MarkdownText::new(md)
            .render()
            .into_iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn renders_headers() {
        let out = render_all("# One\n## Two\n### Three");
        assert!(out.iter().any(|l| l.contains("One")));
        assert!(out.iter().any(|l| l.contains("Two")));
        assert!(out.iter().any(|l| l.contains("Three")));
    }

    #[test]
    fn renders_inline_code_without_literal_backticks() {
        let out = render_all("This is `code` text");
        assert!(out.iter().any(|l| l.contains("code")));
        assert!(
            !out.iter().any(|l| l.contains('`')),
            "inline code must not emit literal backticks"
        );
    }

    #[test]
    fn bullet_list_uses_bullet_glyph() {
        let out = render_all("- One\n- Two");
        assert!(out.iter().any(|l| l.contains("•") && l.contains("One")));
        assert!(out.iter().any(|l| l.contains("•") && l.contains("Two")));
    }

    #[test]
    fn link_shows_text_then_url_in_parens() {
        let out = render_all("[click](https://example.com)");
        let joined = out.join("\n");
        let click_idx = joined.find("click").expect("link text");
        let url_idx = joined.find("https://example.com").expect("link url");
        assert!(
            click_idx < url_idx,
            "text must appear before URL; got: {joined:?}"
        );
    }

    #[test]
    fn blockquote_gets_prefix() {
        let out = render_all("> quoted");
        assert!(
            out.iter().any(|l| l.contains("│") && l.contains("quoted")),
            "blockquote should be prefixed with `│`; got {out:?}"
        );
    }

    #[test]
    fn table_uses_unicode_box_drawing_with_shared_borders() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |";
        let out = render_all(md);
        let joined = out.join("\n");
        // Top corner + joiner + bottom corner all appear somewhere.
        assert!(joined.contains('┌') && joined.contains('┐'));
        assert!(joined.contains('┴') || joined.contains('┬'));
        assert!(joined.contains('│'));
        // No "double pipe" artefact from the old per-cell concatenation.
        assert!(!joined.contains("||"), "should not emit double pipes: {joined:?}");
    }

    #[test]
    fn no_double_blank_lines_between_paragraphs() {
        let out = render_all("first para\n\nsecond para\n\nthird para");
        let mut consecutive_blanks = 0;
        let mut max_consecutive = 0;
        for line in &out {
            if line.is_empty() {
                consecutive_blanks += 1;
                max_consecutive = max_consecutive.max(consecutive_blanks);
            } else {
                consecutive_blanks = 0;
            }
        }
        assert!(
            max_consecutive <= 1,
            "expected at most one blank line between blocks, got {max_consecutive} blanks"
        );
    }

    #[test]
    fn trailing_blank_line_not_emitted() {
        let out = render_all("hello");
        assert!(
            !out.last().map(|l| l.is_empty()).unwrap_or(false),
            "last line should not be blank: {out:?}"
        );
    }

    #[test]
    fn code_block_respects_max_code_lines_when_not_verbose() {
        let mut body = String::from("```rust\n");
        for i in 0..30 {
            body.push_str(&format!("line {i}\n"));
        }
        body.push_str("```\n");
        let widget = MarkdownText::new(&body); // verbose = false
        let out: Vec<String> = widget
            .render()
            .into_iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect();
        assert!(
            out.iter().any(|l| l.contains("more lines")),
            "truncation hint expected when non-verbose"
        );
    }
}
