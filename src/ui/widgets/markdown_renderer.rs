//! Markdown → `Vec<Line>` state machine.
//!
//! Kept in a sibling file so the public facade in
//! [`markdown`](super::markdown) stays a thin surface. This module
//! walks the `pulldown_cmark` event stream and pushes styled lines
//! through a [`LineSink`] that enforces "at most one consecutive
//! blank line" so the rendered output stays tight.
//!
//! Tables get their own builder in
//! [`markdown_table`](super::markdown_table) — a separate file keeps
//! the grid-drawing math isolated and testable.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::markdown::MarkdownConfig;
use super::markdown_table::render_table;
use crate::ui::theme::Theme;

pub(super) struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    config: &'a MarkdownConfig,
}

/// Accumulator that owns the output `Lines` and drops duplicate blank
/// separators so no block can emit visual noise. Every caller just
/// `push_line` / `push_blank` and trusts the sink to DTRT.
pub(super) struct LineSink {
    lines: Vec<Line<'static>>,
}

impl LineSink {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub(super) fn push_line(&mut self, line: Line<'static>) {
        self.lines.push(line);
    }

    /// Push a blank line only if the previous line wasn't already blank
    /// and at least one real line has been emitted.
    pub(super) fn push_blank(&mut self) {
        match self.lines.last() {
            None => {} // leading blank suppressed
            Some(l) if is_blank(l) => {} // consecutive blank suppressed
            _ => self.lines.push(Line::raw("")),
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        while matches!(self.lines.last(), Some(l) if is_blank(l)) {
            self.lines.pop();
        }
        self.lines
    }
}

fn is_blank(line: &Line<'_>) -> bool {
    line.spans.iter().all(|s| s.content.trim().is_empty())
}

impl<'a> MarkdownRenderer<'a> {
    pub(super) fn new(theme: &'a Theme, config: &'a MarkdownConfig) -> Self {
        Self { theme, config }
    }

    pub(super) fn render(self, content: &str) -> Vec<Line<'static>> {
        let mut sink = LineSink::new();
        let mut current: Vec<Span<'static>> = Vec::new();
        let mut style = Style::default();

        // Code-block state.
        let mut in_code = false;
        let mut code_lang = String::new();
        let mut code_lines: Vec<String> = Vec::new();

        // List state.
        let mut list_depth: usize = 0;
        let mut list_counter: Option<u64> = None;

        // Table state.
        let mut in_table = false;
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();
        let mut current_cell = String::new();

        // Blockquote nesting level — each Start/End of BlockQuote adjusts
        // by one. Used to prefix every quoted line with `│ `.
        let mut quote_depth: usize = 0;

        // Pending link URL — held back so we can emit "text (url)"
        // rather than "url text" (the previous bug).
        let mut pending_link_url: Option<String> = None;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        for event in Parser::new_ext(content, options) {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => {}
                    Tag::Heading { level, .. } => {
                        style = Style::default()
                            .fg(heading_color(self.theme, level))
                            .add_modifier(Modifier::BOLD);
                    }
                    Tag::BlockQuote(_) => {
                        quote_depth += 1;
                        style = Style::default()
                            .fg(self.theme.ui.text_dim)
                            .add_modifier(Modifier::ITALIC);
                    }
                    Tag::CodeBlock(kind) => {
                        in_code = true;
                        code_lang = match kind {
                            CodeBlockKind::Fenced(l) => l.to_string(),
                            _ => String::new(),
                        };
                        code_lines.clear();
                    }
                    Tag::List(start) => {
                        list_depth += 1;
                        list_counter = start;
                    }
                    Tag::Item => {
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        if let Some(n) = list_counter {
                            current.push(Span::styled(
                                format!("{indent}{n}. "),
                                Style::default().fg(self.theme.ui.text_dim),
                            ));
                            list_counter = Some(n + 1);
                        } else {
                            current.push(Span::styled(
                                format!("{indent}• "),
                                Style::default().fg(self.theme.ui.text_dim),
                            ));
                        }
                    }
                    Tag::Strong => style = style.add_modifier(Modifier::BOLD),
                    Tag::Emphasis => style = style.add_modifier(Modifier::ITALIC),
                    Tag::Strikethrough => style = style.add_modifier(Modifier::CROSSED_OUT),
                    Tag::Link { dest_url, .. } => {
                        // Hold the URL until the link closes; emit the
                        // text (in link colour) first, then `(url)` in
                        // a dim trailing span.
                        pending_link_url = Some(dest_url.to_string());
                        style = Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::UNDERLINED);
                    }
                    Tag::Image { dest_url, .. } => {
                        current.push(Span::styled(
                            format!("[image: {dest_url}]"),
                            Style::default().fg(self.theme.ui.text_dim),
                        ));
                    }
                    Tag::Table(_) => {
                        in_table = true;
                        table_rows.clear();
                    }
                    Tag::TableHead | Tag::TableRow => current_row.clear(),
                    Tag::TableCell => current_cell.clear(),
                    _ => {}
                },
                Event::End(tag) => match tag {
                    TagEnd::Paragraph => {
                        flush_line(&mut sink, &mut current, quote_depth, self.theme);
                        sink.push_blank();
                    }
                    TagEnd::Heading(_) => {
                        flush_line(&mut sink, &mut current, quote_depth, self.theme);
                        style = Style::default();
                        sink.push_blank();
                    }
                    TagEnd::BlockQuote => {
                        flush_line(&mut sink, &mut current, quote_depth, self.theme);
                        quote_depth = quote_depth.saturating_sub(1);
                        style = Style::default();
                        if quote_depth == 0 {
                            sink.push_blank();
                        }
                    }
                    TagEnd::CodeBlock => {
                        emit_code_block(
                            &mut sink,
                            self.theme,
                            self.config,
                            &code_lang,
                            &code_lines,
                        );
                        code_lines.clear();
                        in_code = false;
                        code_lang.clear();
                        sink.push_blank();
                    }
                    TagEnd::List(_) => {
                        list_depth = list_depth.saturating_sub(1);
                        list_counter = None;
                        if list_depth == 0 {
                            sink.push_blank();
                        }
                    }
                    TagEnd::Item => {
                        flush_line(&mut sink, &mut current, quote_depth, self.theme);
                    }
                    TagEnd::Strong => style = style.remove_modifier(Modifier::BOLD),
                    TagEnd::Emphasis => style = style.remove_modifier(Modifier::ITALIC),
                    TagEnd::Strikethrough => style = style.remove_modifier(Modifier::CROSSED_OUT),
                    TagEnd::Link => {
                        if let Some(url) = pending_link_url.take() {
                            current.push(Span::styled(
                                format!(" ({url})"),
                                Style::default().fg(self.theme.ui.text_dim),
                            ));
                        }
                        style = Style::default();
                    }
                    TagEnd::Image => style = Style::default(),
                    TagEnd::Table => {
                        in_table = false;
                        for line in render_table(&table_rows, self.theme) {
                            sink.push_line(line);
                        }
                        sink.push_blank();
                    }
                    TagEnd::TableRow => {
                        table_rows.push(std::mem::take(&mut current_row));
                    }
                    TagEnd::TableCell => {
                        current_row.push(std::mem::take(&mut current_cell));
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code {
                        for line in text.lines() {
                            code_lines.push(line.to_string());
                        }
                    } else if in_table {
                        current_cell.push_str(&text);
                    } else {
                        current.push(Span::styled(text.to_string(), style));
                    }
                }
                Event::Code(code) => {
                    current.push(Span::styled(
                        code.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .bg(Color::Rgb(40, 44, 52)),
                    ));
                }
                Event::Html(html) => current.push(Span::raw(html.to_string())),
                Event::SoftBreak => current.push(Span::raw(" ")),
                Event::HardBreak => flush_line(&mut sink, &mut current, quote_depth, self.theme),
                Event::Rule => {
                    flush_line(&mut sink, &mut current, quote_depth, self.theme);
                    sink.push_line(Line::styled(
                        "─".repeat(self.config.max_width),
                        Style::default().fg(self.theme.ui.border),
                    ));
                    sink.push_blank();
                }
                Event::TaskListMarker(checked) => {
                    let marker = if checked { "☑ " } else { "☐ " };
                    current.push(Span::styled(
                        marker.to_string(),
                        Style::default().fg(self.theme.state.success),
                    ));
                }
                _ => {}
            }
        }

        flush_line(&mut sink, &mut current, quote_depth, self.theme);
        sink.finish()
    }
}

fn heading_color(theme: &Theme, level: HeadingLevel) -> Color {
    match level {
        HeadingLevel::H1 => theme.brand,
        HeadingLevel::H2 => Color::Yellow,
        HeadingLevel::H3 => Color::Cyan,
        _ => theme.ui.text,
    }
}

fn flush_line(
    sink: &mut LineSink,
    current: &mut Vec<Span<'static>>,
    quote_depth: usize,
    theme: &Theme,
) {
    if current.is_empty() {
        return;
    }
    let spans = std::mem::take(current);
    sink.push_line(prefix_for_quote(spans, quote_depth, theme));
}

fn prefix_for_quote(
    mut spans: Vec<Span<'static>>,
    depth: usize,
    theme: &Theme,
) -> Line<'static> {
    if depth == 0 {
        return Line::from(spans);
    }
    let prefix = "│ ".repeat(depth);
    let mut prefixed = Vec::with_capacity(spans.len() + 1);
    prefixed.push(Span::styled(prefix, Style::default().fg(theme.ui.border)));
    prefixed.append(&mut spans);
    Line::from(prefixed)
}

fn emit_code_block(
    sink: &mut LineSink,
    theme: &Theme,
    config: &MarkdownConfig,
    lang: &str,
    lines: &[String],
) {
    let total = lines.len();
    let truncate = !config.verbose && total > config.max_code_lines;

    if !lang.is_empty() {
        sink.push_line(Line::styled(
            format!("» {lang}"),
            Style::default().fg(theme.ui.text_dim),
        ));
    }
    let visible = if truncate {
        &lines[..config.max_code_lines]
    } else {
        lines
    };
    for l in visible {
        sink.push_line(Line::styled(
            format!("  {l}"),
            Style::default().fg(Color::Cyan),
        ));
    }
    if truncate {
        let hidden = total - config.max_code_lines;
        sink.push_line(Line::styled(
            format!("  … {hidden} more lines (Ctrl+O to expand)"),
            Style::default()
                .fg(theme.ui.text_dim)
                .add_modifier(Modifier::ITALIC),
        ));
    }
}
