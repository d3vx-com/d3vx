//! Compact diff preview rendering in the activity panel

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ui::widgets::DiffLineType;

impl App {
    pub(super) fn render_compact_diff_preview(&self, f: &mut Frame, area: Rect, bg_color: Color) {
        f.render_widget(Block::default().style(Style::default().bg(bg_color)), area);

        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        let Some(diff_preview) = self.diff_preview.as_ref() else {
            let empty = Paragraph::new("No diff available for the current workspace.")
                .style(Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color))
                .wrap(Wrap { trim: true });
            f.render_widget(empty, inner);
            return;
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                diff_preview.file_path.clone(),
                Style::default()
                    .fg(self.ui.theme.brand)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  file {}/{}  Ctrl+Left/Right cycle  Ctrl+D full diff",
                    self.selected_diff_index.saturating_add(1),
                    self.git_changes.len().max(1)
                ),
                Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
            ),
        ]));
        lines.push(Line::raw(""));

        for diff_line in diff_preview.visible_lines(inner.height.saturating_sub(2) as usize) {
            let style = match diff_line.line_type {
                DiffLineType::Added => {
                    Style::default()
                        .fg(self.ui.theme.diff.added_text)
                        .bg(self.ui.theme.diff.added)
                }
                DiffLineType::Removed => Style::default()
                    .fg(self.ui.theme.diff.removed_text)
                    .bg(self.ui.theme.diff.removed),
                DiffLineType::Header => Style::default().fg(self.ui.theme.ui.text_dim).bg(bg_color),
                DiffLineType::HunkHeader => Style::default().fg(self.ui.theme.brand).bg(bg_color),
                DiffLineType::Context => Style::default().fg(self.ui.theme.ui.text).bg(bg_color),
            };
            lines.push(Line::from(vec![Span::styled(
                crate::utils::text::truncate(&diff_line.content, inner.width.max(4) as usize),
                style,
            )]));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .style(Style::default().bg(bg_color))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
    }
}
