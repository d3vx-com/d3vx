use super::types::*;
use crate::ipc::{ToolCall, ToolStatus};
use crate::ui::symbols::STATUS;
use crate::ui::theme::get_tool_color;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

impl<'a> MessageList<'a> {
    /// Build lines for tool calls
    pub(crate) fn build_tool_call_lines(
        &self,
        tool_calls: &[ToolCall],
        lines: &mut Vec<Line<'static>>,
    ) {
        if self.verbose {
            for tool in tool_calls {
                self.build_single_tool_call_lines(tool, lines);
            }
        } else {
            if tool_calls.len() > 1 {
                let completed = tool_calls
                    .iter()
                    .filter(|t| t.status == ToolStatus::Completed)
                    .count();
                lines.push(Line::from(Span::styled(
                    format!(
                        "  {} {}/{} completed",
                        STATUS.success,
                        completed,
                        tool_calls.len()
                    ),
                    Style::default().fg(self.theme.state.success),
                )));
            }

            if let Some(latest) = tool_calls.last() {
                self.build_single_tool_call_lines(latest, lines);
            }
        }
    }

    /// Build lines for a single tool call
    fn build_single_tool_call_lines(&self, tool: &ToolCall, lines: &mut Vec<Line<'static>>) {
        let status_icon = match tool.status {
            ToolStatus::Pending => STATUS.pending,
            ToolStatus::Running => STATUS.running,
            ToolStatus::Completed => STATUS.success,
            ToolStatus::Error => STATUS.error,
            ToolStatus::WaitingApproval => STATUS.pending,
        };

        let tool_color = get_tool_color(&tool.name);
        let summary = self.get_tool_summary(&tool.name, &tool.input);

        let mut spans = vec![
            Span::styled(
                format!("  {} ", status_icon),
                Style::default().fg(self.theme.state.success),
            ),
            Span::styled(
                tool.name.clone(),
                Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
            ),
        ];

        if let Some(s) = summary {
            spans.push(Span::styled(
                format!(" {}", s),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }

        if let Some(elapsed) = tool.elapsed {
            if elapsed > 0 {
                spans.push(Span::styled(
                    format!(" {}ms", elapsed),
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
        }

        lines.push(Line::from(spans));

        if self.verbose {
            if let Some(output) = &tool.output {
                let out_lines: Vec<&str> = output.lines().collect();
                let max_lines = self.truncate.tool_output_lines;

                if out_lines.len() <= max_lines {
                    for line in out_lines {
                        lines.push(Line::styled(
                            format!("    {}", line),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                } else {
                    let head_count = max_lines - 3;
                    for line in out_lines.iter().take(head_count) {
                        lines.push(Line::styled(
                            format!("    {}", line),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }

                    let hidden = out_lines.len() - head_count - 3;
                    lines.push(Line::styled(
                        format!("    ... {} lines hidden ...", hidden),
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));

                    for line in out_lines.iter().rev().take(3).rev() {
                        lines.push(Line::styled(
                            format!("    {}", line),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                }
            }
        }

        if tool.status == ToolStatus::Error {
            if let Some(output) = &tool.output {
                let truncated = if output.len() > 300 {
                    format!("{}...", &output[..297])
                } else {
                    output.clone()
                };
                lines.push(Line::styled(
                    format!("    {}", truncated),
                    Style::default().fg(self.theme.state.error),
                ));
            }
        }
    }

    /// Generate a human-readable summary for a tool
    fn get_tool_summary(&self, tool_name: &str, input: &serde_json::Value) -> Option<String> {
        let get_str = |key: &str| -> String {
            input
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let truncate_str = |s: &str, max: usize| -> String {
            if s.len() > max {
                format!("{}...", &s[..max.saturating_sub(3)])
            } else {
                s.to_string()
            }
        };

        match tool_name {
            "BashTool" | "Bash" => Some(truncate_str(&get_str("command"), self.truncate.command)),
            "ReadTool" => {
                let path = get_str("file_path");
                let offset = input
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .map(|o| format!(":{}", o))
                    .unwrap_or_default();
                Some(truncate_str(
                    &format!("{}{}", path, offset),
                    self.truncate.path,
                ))
            }
            "WriteTool" | "EditTool" | "MultiEditTool" => {
                Some(truncate_str(&get_str("file_path"), self.truncate.path))
            }
            "GrepTool" => {
                let pattern = get_str("pattern");
                let path = get_str("path");
                let suffix = if path.is_empty() {
                    "".to_string()
                } else {
                    format!(" in {}", path)
                };
                Some(truncate_str(
                    &format!("\"{}\"{}", pattern, suffix),
                    self.truncate.path,
                ))
            }
            "GlobTool" => {
                let pattern = get_str("pattern");
                let path = get_str("path");
                let suffix = if path.is_empty() {
                    "".to_string()
                } else {
                    format!(" in {}", path)
                };
                Some(truncate_str(
                    &format!("\"{}\"{}", pattern, suffix),
                    self.truncate.path,
                ))
            }
            "WebSearchTool" | "webSearchTool" => {
                Some(truncate_str(&get_str("query"), self.truncate.query))
            }
            "WebFetchTool" => Some(truncate_str(&get_str("url"), self.truncate.url)),
            "ThinkTool" => Some("reasoning...".to_string()),
            "TodoWriteTool" => Some("updating todo list".to_string()),
            _ => {
                if let Some(obj) = input.as_object() {
                    if let Some((key, value)) = obj.iter().next() {
                        Some(truncate_str(
                            &format!("{}: {}", key, value),
                            self.truncate.query,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
}
