//! Thinking Indicator Widget
//!
//! Animated spinner with thinking text, displayed above the input box.
//!
//! Features:
//! - Animated dot loader frames
//! - Elapsed time display
//! - Phase breadcrumbs (Research → Plan → Implement → Validate)
//! - Tool summary when running tools

use crate::ipc::{ToolCall, ToolStatus};
use crate::ui::symbols::SPINNER_DOTS;
use crate::ui::theme::Theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Thinking verbs to cycle through
const THINKING_VERBS: &[&str] = &[
    "Thinking",
    "Analysing",
    "Reasoning",
    "Computing",
    "Deliberating",
    "Contemplating",
    "Evaluating",
    "Considering",
];

/// Tips to show after waiting
const TIPS: &[&str] = &[
    "Tip: Press Ctrl+C to interrupt",
    "Tip: Use /help to see available commands",
    "Tip: Ctrl+O toggles verbose mode",
    "Tip: Ctrl+B shows diff preview",
    "Tip: Esc twice opens undo picker",
];

/// Phase names for breadcrumbs (internal → user-facing mapping)
const PHASE_DISPLAY_NAMES: &[(&str, &str)] = &[
    ("Research", "Understanding"),
    ("Ideation", "Exploring"),
    ("Plan", "Planning"),
    ("Draft", "Writing code"),
    ("Implement", "Implementing"),
    ("Review", "Checking"),
    ("Docs", "Documenting"),
    ("Validate", "Validating"),
];

/// Get user-facing name for a pipeline phase
fn user_facing_phase(internal: &str) -> &str {
    PHASE_DISPLAY_NAMES
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(internal))
        .map(|(_, display)| *display)
        .unwrap_or(internal)
}

/// Ordered phase list for breadcrumbs (using user-facing names)
const PHASES: &[&str] = &[
    "Understanding",
    "Planning",
    "Writing code",
    "Implementing",
    "Checking",
    "Documenting",
];

/// Thinking indicator configuration (owned)
pub struct ThinkingIndicator {
    /// Custom text (overrides verb)
    text: Option<String>,
    /// Current tool being executed
    tool_name: Option<String>,
    /// Tool input for summary
    tool_input: Option<serde_json::Value>,
    /// Tool status
    tool_status: ToolStatus,
    /// Current phase (for breadcrumbs)
    phase: Option<String>,
    /// Animation frame index
    frame: usize,
    /// Elapsed time in seconds
    elapsed_secs: u64,
    /// Theme
    theme: Theme,
    /// Number of active sub-agents
    subagent_count: usize,
    /// Number of active background tasks (Vex/Autonomous)
    background_task_count: usize,
}

impl ThinkingIndicator {
    /// Create a new thinking indicator
    pub fn new() -> Self {
        Self {
            text: None,
            tool_name: None,
            tool_input: None,
            tool_status: ToolStatus::Pending,
            phase: None,
            frame: 0,
            elapsed_secs: 0,
            theme: Theme::dark(),
            subagent_count: 0,
            background_task_count: 0,
        }
    }

    /// Set sub-agent count
    pub fn subagent_count(mut self, count: usize) -> Self {
        self.subagent_count = count;
        self
    }

    /// Set background task count
    pub fn background_task_count(mut self, count: usize) -> Self {
        self.background_task_count = count;
        self
    }

    /// Set custom text
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Set current tool
    pub fn tool(mut self, tool: &ToolCall) -> Self {
        self.tool_name = Some(tool.name.clone());
        self.tool_input = Some(tool.input.clone());
        self.tool_status = tool.status;
        self
    }

    /// Set current phase
    pub fn phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    /// Set animation frame
    pub fn frame(mut self, frame: usize) -> Self {
        self.frame = frame;
        self
    }

    /// Set elapsed time in seconds
    pub fn elapsed(mut self, secs: u64) -> Self {
        self.elapsed_secs = secs;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Get the current spinner frame character (owned)
    fn get_spinner_char(&self) -> String {
        SPINNER_DOTS[self.frame % SPINNER_DOTS.len()].to_string()
    }

    /// Get a tip based on elapsed time
    fn get_tip(&self) -> Option<String> {
        if self.elapsed_secs >= 3 {
            let idx = ((self.elapsed_secs - 3) / 5) as usize % TIPS.len();
            Some(TIPS[idx].to_string())
        } else {
            None
        }
    }

    /// Format elapsed time
    fn format_elapsed(&self) -> String {
        if self.elapsed_secs < 60 {
            format!("{}s", self.elapsed_secs)
        } else {
            let mins = self.elapsed_secs / 60;
            let secs = self.elapsed_secs % 60;
            format!("{}m {}s", mins, secs)
        }
    }

    /// Get compact tool summary
    fn get_tool_summary(&self) -> String {
        let input = match &self.tool_input {
            Some(i) => i,
            None => return String::new(),
        };

        let truncate = |s: &str, max: usize| -> String {
            if s.len() > max {
                format!("{}...", &s[..max.saturating_sub(3)])
            } else {
                s.to_string()
            }
        };

        let get_str = |key: &str| -> String {
            input
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        match self.tool_name.as_deref() {
            Some("BashTool") | Some("Bash") => format!("$ {}", truncate(&get_str("command"), 50)),
            Some("ReadTool") | Some("WriteTool") | Some("EditTool") | Some("MultiEditTool") => {
                truncate(&get_str("file_path"), 50)
            }
            Some("GrepTool") | Some("GlobTool") => {
                format!("\"{}\"", truncate(&get_str("pattern"), 40))
            }
            Some("WebSearchTool") | Some("webSearchTool") => truncate(&get_str("query"), 40),
            Some("WebFetchTool") => truncate(&get_str("url"), 40),
            _ => {
                // Show first key-value pair
                if let Some(obj) = input.as_object() {
                    if let Some((key, value)) = obj.iter().next() {
                        truncate(&format!("{}: {}", key, value), 40)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        }
    }

    /// Render the thinking indicator into lines (all owned)
    pub fn build_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let brand_color = self.theme.brand;

        // Detect Vex Mode
        let is_vex_mode = self.text.as_deref() == Some("Vex Mode Active");

        // Mascot prefix for Vex Mode
        if is_vex_mode {
            let mascot_lines = vec!["  ▄▄▄▄▄▄▄  ", "  █ ▀ ▀ █  ", " ▀█▄▄▄▄▄█▀ "];
            for line in mascot_lines {
                lines.push(Line::from(vec![Span::styled(
                    line,
                    Style::default()
                        .fg(brand_color)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
        }

        // Determine display text
        let show_tool = self.tool_name.is_some() && self.tool_status == ToolStatus::Running;
        let display_text = if show_tool {
            format!("Running {}...", self.tool_name.as_deref().unwrap_or("tool"))
        } else if self.text.is_some() {
            format!("{}...", self.text.as_deref().unwrap_or("Processing"))
        } else {
            let verb = {
                let idx = (self.elapsed_secs / 3) as usize % THINKING_VERBS.len();
                THINKING_VERBS[idx]
            };
            if self.subagent_count > 0 || self.background_task_count > 0 {
                let total = self.subagent_count + self.background_task_count;
                format!(
                    "Claude is managing {} background task{}...",
                    total,
                    if total > 1 { "s" } else { "" }
                )
            } else {
                format!("{}...", verb)
            }
        };

        // Main line: spinner + text + elapsed time
        let mut spans = vec![
            Span::styled(self.get_spinner_char(), Style::default().fg(brand_color)),
            Span::raw(" "),
            Span::styled(
                display_text,
                Style::default()
                    .fg(brand_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        // Add elapsed time and interrupt hint
        spans.push(Span::styled(
            format!(" ({} ", self.format_elapsed()),
            Style::default().fg(self.theme.ui.text_dim),
        ));
        spans.push(Span::styled(
            "·",
            Style::default().fg(self.theme.ui.text_dim),
        ));
        spans.push(Span::styled(
            " esc to interrupt)",
            Style::default().fg(self.theme.ui.text_dim),
        ));

        lines.push(Line::from(spans));

        // Phase breadcrumbs (user-facing names)
        if let Some(current_phase) = &self.phase {
            let mut phase_spans = vec![Span::raw("  ")];
            let display_name = user_facing_phase(current_phase);

            for (i, phase) in PHASES.iter().enumerate() {
                let is_current = phase == &display_name;
                let color = if is_current {
                    brand_color
                } else {
                    self.theme.ui.text_dim
                };
                let style = Style::default().fg(color).add_modifier(if is_current {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });

                phase_spans.push(Span::styled(phase.to_string(), style));

                if i < PHASES.len() - 1 {
                    phase_spans.push(Span::styled(
                        " → ",
                        Style::default().fg(self.theme.ui.text_dim),
                    ));
                }
            }

            lines.push(Line::from(phase_spans));
        }

        // Tool summary if running
        if show_tool {
            let summary = self.get_tool_summary();
            if !summary.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(summary, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
        }

        // Tips after 3 seconds (only if not running a tool)
        if !show_tool {
            if let Some(tip) = self.get_tip() {
                lines.push(Line::from(vec![
                    Span::styled("  🙹 ", Style::default().fg(self.theme.ui.text_dim)),
                    Span::styled(tip, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
        }

        lines
    }
}

impl Default for ThinkingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ThinkingIndicator {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let lines = self.build_lines();
        for (i, line) in lines.iter().enumerate() {
            if i < area.height as usize {
                let y = area.y + i as u16;
                if y < buf.area.height {
                    line.render(ratatui::layout::Rect::new(area.x, y, area.width, 1), buf);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_indicator_displays_text() {
        let indicator = ThinkingIndicator::new()
            .text("Thinking")
            .frame(0)
            .elapsed(1);

        let lines = indicator.build_lines();
        assert!(lines[0]
            .spans
            .iter()
            .any(|s| s.content.contains("Thinking")));
    }

    #[test]
    fn test_elapsed_time_format() {
        let indicator = ThinkingIndicator::new().elapsed(65);
        let lines = indicator.build_lines();
        assert!(lines[0].spans.iter().any(|s| s.content.contains("1m 5s")));
    }
}
