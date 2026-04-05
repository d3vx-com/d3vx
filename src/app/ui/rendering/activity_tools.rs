//! Activity panel helpers: git changes rendering and agent icon utilities

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::state::InlineAgentStatus;
use crate::app::App;
use crate::ui::theme::Theme;

use crate::app::ui::helpers::{braille_frame, MUTED_WHITE};

impl App {
    /// Render git changes section in the activity panel summary
    pub(super) fn render_activity_git_changes(
        &mut self,
        summary_lines: &mut Vec<Line<'_>>,
        mut current_line: usize,
        summary_area: Rect,
    ) {
        if !self.git_changes.is_empty() {
            summary_lines.push(Line::from(vec![Span::styled(
                format!("Changed Files ({})", self.git_changes.len()),
                Style::default()
                    .fg(self.ui.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            current_line += 1;
            self.layout.activity_diff_y_positions.clear();
            for change in self.git_changes.iter().take(5) {
                let file_name = std::path::Path::new(&change.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&change.path);
                self.layout.activity_diff_y_positions.push(current_line);
                summary_lines.push(Line::from(vec![
                    Span::styled(" \u{2022} ", Style::default().fg(self.ui.theme.brand)),
                    Span::styled(
                        crate::utils::text::truncate(
                            file_name,
                            summary_area.width.saturating_sub(4) as usize,
                        ),
                        Style::default().fg(self.ui.theme.ui.text),
                    ),
                ]));
                current_line += 1;
            }
        }
    }
}

/// Get the status icon and color for an inline agent in the activity panel
pub(super) fn inline_agent_icon_color(status: InlineAgentStatus, theme: &Theme) -> (String, Color) {
    match status {
        InlineAgentStatus::Running => (braille_frame().to_string(), MUTED_WHITE),
        InlineAgentStatus::Completed => (
            crate::ui::symbols::STATUS.success.to_string(),
            Color::Rgb(80, 200, 120),
        ),
        InlineAgentStatus::Ended => ("\u{2500}".to_string(), Color::Rgb(100, 180, 120)),
        InlineAgentStatus::Failed => (
            crate::ui::symbols::STATUS.error.to_string(),
            Color::Rgb(220, 100, 100),
        ),
        InlineAgentStatus::Cancelled => ("\u{2500}".to_string(), theme.ui.text_dim),
    }
}
