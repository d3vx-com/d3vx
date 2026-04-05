//! Tool display rendering logic

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};

use super::types::ToolDisplay;
use crate::ipc::ToolStatus;
use crate::ui::symbols::{get_tool_icon, SPINNER_DOTS, STATUS};
use crate::ui::theme::get_tool_color;

impl<'a> ToolDisplay<'a> {
    /// Get status icon based on state
    pub(crate) fn get_status_icon(&self) -> &'static str {
        match self.status {
            ToolStatus::Pending => STATUS.pending,
            ToolStatus::Running => {
                let frames = SPINNER_DOTS;
                frames[self.animation_frame as usize % frames.len()]
            }
            ToolStatus::Completed => STATUS.success,
            ToolStatus::Error => STATUS.error,
            ToolStatus::WaitingApproval => STATUS.pending,
        }
    }

    /// Get status color
    pub(crate) fn get_status_color(&self) -> ratatui::style::Color {
        match self.status {
            ToolStatus::Pending => self.theme.state.pending,
            ToolStatus::Running => self.theme.brand,
            ToolStatus::Completed => self.theme.state.success,
            ToolStatus::Error => self.theme.state.error,
            ToolStatus::WaitingApproval => self.theme.state.pending,
        }
    }

    /// Generate a summary of the tool input
    pub(crate) fn get_input_summary(&self) -> String {
        let get_str = |key: &str| -> String {
            self.input
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let truncate = |s: &str, max: usize| -> String {
            if s.len() > max {
                format!("{}...", &s[..max.saturating_sub(3)])
            } else {
                s.to_string()
            }
        };

        match self.name {
            "BashTool" | "Bash" => truncate(&get_str("command"), self.config.max_input_preview),
            "ReadTool" | "Read" => {
                let path = get_str("file_path");
                let offset = self
                    .input
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .map(|o| format!(":{}", o))
                    .unwrap_or_default();
                truncate(
                    &format!("{}{}", path, offset),
                    self.config.max_input_preview,
                )
            }
            "WriteTool" | "Write" => truncate(&get_str("file_path"), self.config.max_input_preview),
            "EditTool" | "MultiEditTool" => {
                truncate(&get_str("file_path"), self.config.max_input_preview)
            }
            "GrepTool" | "Grep" => {
                let pattern = get_str("pattern");
                let path = get_str("path");
                let suffix = if path.is_empty() {
                    "".to_string()
                } else {
                    format!(" in {}", path)
                };
                truncate(
                    &format!("\"{}\"{}", pattern, suffix),
                    self.config.max_input_preview,
                )
            }
            "GlobTool" | "Glob" => {
                let pattern = get_str("pattern");
                truncate(&format!("\"{}\"", pattern), self.config.max_input_preview)
            }
            "WebFetchTool" => truncate(&get_str("url"), self.config.max_input_preview),
            "ThinkTool" => "reasoning...".to_string(),
            "TodoWriteTool" => "updating todos".to_string(),
            "QuestionTool" => truncate(&get_str("question"), self.config.max_input_preview),
            _ => {
                if let Some(obj) = self.input.as_object() {
                    if let Some((key, value)) = obj.iter().next() {
                        truncate(
                            &format!("{}: {}", key, value),
                            self.config.max_input_preview,
                        )
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        }
    }

    /// Build lines for this tool display
    pub fn build_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let tool_color = get_tool_color(self.name);
        let status_color = self.get_status_color();
        let status_icon = self.get_status_icon();
        let tool_icon = get_tool_icon(self.name);
        let input_summary = self.get_input_summary();

        // Header line
        let mut header_spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                status_icon,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(tool_icon, Style::default()),
            Span::styled(" ", Style::default()),
            Span::styled(
                self.name.to_string(),
                Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
            ),
        ];

        if !self.config.verbose && !input_summary.is_empty() {
            header_spans.push(Span::styled(
                format!(" {}", input_summary),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }

        if self.config.show_timing {
            if let Some(elapsed) = self.elapsed_ms {
                if elapsed > 0 {
                    let time_str = if elapsed < 1000 {
                        format!(" {}ms", elapsed)
                    } else {
                        format!(" {:.1}s", elapsed as f64 / 1000.0)
                    };
                    header_spans.push(Span::styled(
                        time_str,
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
            }
        }

        lines.push(Line::from(header_spans));

        // Input section (full JSON in verbose mode)
        if self.config.verbose {
            let json_str = serde_json::to_string_pretty(self.input).unwrap_or_default();
            for line in json_str.lines() {
                lines.push(Line::styled(
                    format!("      {}", line),
                    Style::default().fg(self.theme.ui.text_dim),
                ));
            }
        }

        // Output section
        self.render_output(&mut lines);

        lines
    }

    /// Render output lines (errors or verbose output)
    fn render_output(&self, lines: &mut Vec<Line<'static>>) {
        if !self.config.verbose && self.status != ToolStatus::Error {
            return;
        }

        let Some(output) = self.output else { return };

        let is_error = self.status == ToolStatus::Error;
        let color = if is_error {
            self.theme.state.error
        } else {
            self.theme.ui.text_dim
        };
        let prefix = if is_error { "Error: " } else { "" };

        let mut output_to_render = output.to_string();

        // Try JSON pretty-print
        if (output.trim_start().starts_with('{') || output.trim_start().starts_with('['))
            && output.len() < 10000
        {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    output_to_render = pretty;
                }
            }
        }

        let all_lines: Vec<&str> = output_to_render.lines().collect();
        let non_empty_lines: Vec<&str> = all_lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let truncate_line = |s: &str| -> String {
            let s = s.trim_end();
            if s.len() > self.config.max_width.saturating_sub(10) {
                format!("{}...", &s[..self.config.max_width.saturating_sub(13)])
            } else {
                s.to_string()
            }
        };

        if all_lines.len() <= 3 || self.config.verbose {
            let limit = self.config.max_output_lines.min(all_lines.len());
            for line in all_lines.iter().take(limit) {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}{}", prefix, truncate_line(line)),
                        Style::default().fg(color),
                    ),
                ]));
            }
            if all_lines.len() > limit {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("... ({} more lines) ...", all_lines.len() - limit),
                        Style::default()
                            .fg(self.theme.ui.text_muted)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        } else {
            // Summarized mode: First non-empty, hidden count, Last non-empty
            if let Some(first) = non_empty_lines.first() {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}{}", prefix, truncate_line(first)),
                        Style::default().fg(color),
                    ),
                ]));

                let hidden = all_lines.len().saturating_sub(2);
                if hidden > 0 {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("... ({} lines hidden) ...", hidden),
                            Style::default()
                                .fg(self.theme.ui.text_muted)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }

                if let Some(last) = non_empty_lines.last() {
                    if non_empty_lines.len() > 1 {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(
                                format!("{}{}", prefix, truncate_line(last)),
                                Style::default().fg(color),
                            ),
                        ]));
                    }
                }
            }
        }
    }
}

impl<'a> Widget for ToolDisplay<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let lines = self.build_lines();
        let paragraph = Paragraph::new(Text::from(lines));
        paragraph.render(area, buf);
    }
}
