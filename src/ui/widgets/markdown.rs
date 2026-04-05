//! Markdown Text Widget
//!
//! Renders markdown text with terminal formatting.
//!
//! Features:
//! - Headers with hierarchy
//! - Bold, italic, inline code
//! - Code blocks with language labels
//! - Blockquotes
//! - Lists (bullet and numbered)

use crate::ui::theme::Theme;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Markdown renderer configuration
pub struct MarkdownConfig {
    /// Maximum width for text wrapping
    pub max_width: usize,
    /// Whether to show full content (no truncation)
    pub verbose: bool,
    /// Maximum lines for code blocks (non-verbose)
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

/// Markdown text widget
pub struct MarkdownText {
    content: String,
    config: MarkdownConfig,
    theme: Theme,
}

impl MarkdownText {
    /// Create a new markdown text widget
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            config: MarkdownConfig::default(),
            theme: Theme::dark(),
        }
    }

    /// Set configuration
    pub fn config(mut self, config: MarkdownConfig) -> Self {
        self.config = config;
        self
    }

    /// Set verbose mode
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Render markdown to lines
    pub fn render(&self) -> Vec<Line<'static>> {
        let renderer = MarkdownRenderer::new(&self.theme, &self.config);
        renderer.render(&self.content)
    }
}

/// Internal markdown renderer state
struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    config: &'a MarkdownConfig,
}

impl<'a> MarkdownRenderer<'a> {
    fn new(theme: &'a Theme, config: &'a MarkdownConfig) -> Self {
        Self { theme, config }
    }

    fn render(self, content: &str) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut current_style = Style::default();
        let mut list_depth: usize = 0;
        let mut list_counter: Option<u64> = None;
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_lines: Vec<String> = Vec::new();
        let mut in_table = false;
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();
        let mut current_cell = String::new();

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(content, options);

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            // Just continue
                        }
                        Tag::Heading { level, .. } => {
                            let color = match level {
                                HeadingLevel::H1 => self.theme.brand,
                                HeadingLevel::H2 => Color::Yellow,
                                HeadingLevel::H3 => Color::Cyan,
                                _ => self.theme.ui.text,
                            };
                            current_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
                        }
                        Tag::BlockQuote(_kind) => {
                            current_style = Style::default()
                                .fg(self.theme.ui.text_dim)
                                .add_modifier(Modifier::ITALIC);
                        }
                        Tag::CodeBlock(kind) => {
                            in_code_block = true;
                            code_block_lang = match kind {
                                CodeBlockKind::Fenced(lang) => lang.to_string(),
                                _ => String::new(),
                            };
                            code_block_lines.clear();
                        }
                        Tag::List(start) => {
                            list_depth += 1;
                            list_counter = start;
                        }
                        Tag::Item => {
                            let indent = "  ".repeat(list_depth.saturating_sub(1));
                            if let Some(counter) = list_counter {
                                current_spans.push(Span::styled(
                                    format!("{}{}. ", indent, counter),
                                    Style::default().fg(Color::Yellow),
                                ));
                                list_counter = Some(counter + 1);
                            } else {
                                current_spans.push(Span::styled(
                                    format!("{}• ", indent),
                                    Style::default().fg(Color::Cyan),
                                ));
                            }
                        }
                        Tag::Strong => {
                            current_style = current_style.add_modifier(Modifier::BOLD);
                        }
                        Tag::Emphasis => {
                            current_style = current_style.add_modifier(Modifier::ITALIC);
                        }
                        Tag::Strikethrough => {
                            current_style = current_style.add_modifier(Modifier::CROSSED_OUT);
                        }
                        Tag::Link {
                            link_type: _,
                            dest_url,
                            title: _,
                            id: _,
                        } => {
                            // Store link for display
                            current_style = Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::UNDERLINED);
                            current_spans.push(Span::styled(dest_url.to_string(), current_style));
                        }
                        Tag::Image {
                            link_type: _,
                            dest_url,
                            title: _,
                            id: _,
                        } => {
                            current_spans.push(Span::styled(
                                format!("[Image: {}]", dest_url),
                                Style::default().fg(self.theme.ui.text_dim),
                            ));
                        }
                        Tag::Table(_) => {
                            in_table = true;
                            table_rows.clear();
                        }
                        Tag::TableHead => {}
                        Tag::TableRow => {
                            current_row.clear();
                        }
                        Tag::TableCell => {
                            current_cell.clear();
                        }
                        _ => {}
                    }
                }
                Event::End(tag) => {
                    match tag {
                        TagEnd::Table => {
                            in_table = false;
                            // Render table
                            if !table_rows.is_empty() {
                                let num_cols = table_rows[0].len();
                                let mut col_widths = vec![0; num_cols];
                                for row in &table_rows {
                                    for (i, cell) in row.iter().enumerate() {
                                        if i < num_cols {
                                            col_widths[i] = col_widths[i].max(cell.len());
                                        }
                                    }
                                }

                                // Separator line
                                let mut separator_spans = Vec::new();
                                for (_i, &width) in col_widths.iter().enumerate() {
                                    separator_spans.push(Span::styled(
                                        format!("+{}+", "-".repeat(width + 2)),
                                        Style::default().fg(self.theme.ui.border),
                                    ));
                                }
                                lines.push(Line::from(separator_spans));

                                for (r, row) in table_rows.iter().enumerate() {
                                    let mut row_spans = Vec::new();
                                    for (i, cell) in row.iter().enumerate() {
                                        if i < num_cols {
                                            let padding = col_widths[i] - cell.len();
                                            let cell_text =
                                                format!("| {} {} |", cell, " ".repeat(padding));
                                            let style = if r == 0 {
                                                Style::default()
                                                    .add_modifier(Modifier::BOLD)
                                                    .fg(self.theme.brand)
                                            } else {
                                                Style::default().fg(self.theme.ui.text)
                                            };
                                            row_spans.push(Span::styled(cell_text, style));
                                        }
                                    }
                                    lines.push(Line::from(row_spans));

                                    // Separator after header
                                    if r == 0 {
                                        let mut header_sep_spans = Vec::new();
                                        for &width in &col_widths {
                                            header_sep_spans.push(Span::styled(
                                                format!("+{}+", "-".repeat(width + 2)),
                                                Style::default().fg(self.theme.ui.border),
                                            ));
                                        }
                                        lines.push(Line::from(header_sep_spans));
                                    }
                                }

                                // Final separator
                                let mut final_sep_spans = Vec::new();
                                for &width in &col_widths {
                                    final_sep_spans.push(Span::styled(
                                        format!("+{}+", "-".repeat(width + 2)),
                                        Style::default().fg(self.theme.ui.border),
                                    ));
                                }
                                lines.push(Line::from(final_sep_spans));
                                lines.push(Line::raw(""));
                            }
                        }
                        TagEnd::TableRow => {
                            table_rows.push(std::mem::take(&mut current_row));
                        }
                        TagEnd::TableCell => {
                            current_row.push(std::mem::take(&mut current_cell));
                        }
                        TagEnd::Paragraph => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                            lines.push(Line::raw("")); // Spacing
                        }
                        TagEnd::Heading(_) => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                            current_style = Style::default();
                        }
                        TagEnd::BlockQuote => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                            current_style = Style::default();
                        }
                        TagEnd::CodeBlock => {
                            // Flush code block
                            let total_lines = code_block_lines.len();
                            let should_truncate =
                                !self.config.verbose && total_lines > self.config.max_code_lines;

                            if !code_block_lang.is_empty() {
                                lines.push(Line::styled(
                                    format!("[ {} ]", code_block_lang),
                                    Style::default().fg(self.theme.ui.text_dim),
                                ));
                            }

                            if should_truncate {
                                for line in &code_block_lines[..self.config.max_code_lines] {
                                    lines.push(Line::styled(
                                        format!("  {}", line),
                                        Style::default().fg(Color::Cyan),
                                    ));
                                }
                                let hidden = total_lines - self.config.max_code_lines;
                                lines.push(Line::styled(
                                    format!("  ... {} more lines (Ctrl+O to expand)", hidden),
                                    Style::default().add_modifier(Modifier::ITALIC),
                                ));
                            } else {
                                for line in &code_block_lines {
                                    lines.push(Line::styled(
                                        format!("  {}", line),
                                        Style::default().fg(Color::Cyan),
                                    ));
                                }
                            }

                            code_block_lines.clear();
                            in_code_block = false;
                            code_block_lang.clear();
                        }
                        TagEnd::List(_) => {
                            list_depth = list_depth.saturating_sub(1);
                            list_counter = None;
                        }
                        TagEnd::Item => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                        }
                        TagEnd::Strong => {
                            current_style = current_style.remove_modifier(Modifier::BOLD);
                        }
                        TagEnd::Emphasis => {
                            current_style = current_style.remove_modifier(Modifier::ITALIC);
                        }
                        TagEnd::Strikethrough => {
                            current_style = current_style.remove_modifier(Modifier::CROSSED_OUT);
                        }
                        TagEnd::Link => {
                            current_style = Style::default();
                        }
                        TagEnd::Image => {
                            current_style = Style::default();
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    if in_code_block {
                        for line in text.lines() {
                            code_block_lines.push(line.to_string());
                        }
                    } else if in_table {
                        current_cell.push_str(&text);
                    } else {
                        current_spans.push(Span::styled(text.to_string(), current_style));
                    }
                }
                Event::Code(code) => {
                    let style = Style::default().fg(Color::Cyan).bg(Color::Rgb(40, 44, 52));
                    current_spans.push(Span::styled(format!("`{}`", code), style));
                }
                Event::Html(html) => {
                    current_spans.push(Span::raw(html.to_string()));
                }
                Event::SoftBreak => {
                    current_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                Event::Rule => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::styled(
                        "─".repeat(self.config.max_width.min(40)),
                        Style::default().fg(self.theme.ui.border),
                    ));
                }
                Event::TaskListMarker(checked) => {
                    let marker = if checked { "☑ " } else { "☐ " };
                    current_spans.push(Span::styled(
                        marker.to_string(),
                        Style::default().fg(self.theme.state.success),
                    ));
                }
                _ => {}
            }
        }

        // Flush remaining spans
        if !current_spans.is_empty() {
            lines.push(Line::from(current_spans));
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_headers() {
        let md = "# Header 1\n## Header 2\n### Header 3";
        let widget = MarkdownText::new(md);
        let lines = widget.render();

        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_markdown_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let widget = MarkdownText::new(md);
        let lines = widget.render();

        assert!(lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("rust"))));
    }

    #[test]
    fn test_markdown_bold() {
        let md = "This is **bold** text";
        let widget = MarkdownText::new(md);
        let lines = widget.render();

        assert!(lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("bold"))));
    }

    #[test]
    fn test_markdown_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let widget = MarkdownText::new(md);
        let lines = widget.render();

        assert!(lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("•"))));
    }
}
