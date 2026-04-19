//! Detail Drawer — full-width agent output area in the main chat column

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

impl App {
    /// Render the detail drawer — full-width agent output below the chat area.
    ///
    /// Reuses `render_selected_agent_detail` by temporarily swapping the
    /// selected agent and scroll state.
    pub fn render_drawer(&mut self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        // Determine which agent to show
        let agent_idx = if let Some(ref id) = self.ui.drawer_agent_id {
            self.agents.inline_agents.iter().position(|a| &a.id == id)
        } else {
            self.agents.selected_inline_agent
        };

        // Draw a top border to separate from chat
        let border_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 75)));
        let inner = border_block.inner(area);
        f.render_widget(border_block, area);

        // Header row + content area
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);

        // Header: agent name + controls hint
        let header_text = if let Some(idx) = agent_idx {
            if idx < self.agents.inline_agents.len() {
                let agent = &self.agents.inline_agents[idx];
                let is_bg = agent.id.starts_with("vex:");
                let prefix = if is_bg { "Background: " } else { "" };
                let max_len = chunks[0].width.saturating_sub(4) as usize;
                let task = if agent.task.len() > max_len {
                    format!("{}..", &agent.task[..max_len.saturating_sub(2)])
                } else {
                    agent.task.clone()
                };
                format!(
                    " {}{}{} [{}] Ctrl+W cycle | Esc close",
                    prefix,
                    task,
                    if is_bg { "" } else { "" },
                    agent.elapsed()
                )
            } else {
                " Agent not found ".to_string()
            }
        } else {
            " No agent selected — click an agent in the strip above ".to_string()
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                header_text,
                Style::default().fg(self.ui.theme.ui.text_dim),
            )])),
            chunks[0],
        );

        // Render agent detail content — reuse existing method with state swap
        let prev_selected = self.agents.selected_inline_agent;
        let prev_scroll = self.ui.selected_agent_output_scroll;

        // Set agent for detail rendering
        if let Some(idx) = agent_idx {
            self.agents.selected_inline_agent = Some(idx);
        }
        self.ui.selected_agent_output_scroll = self.ui.drawer_scroll;

        self.render_selected_agent_detail(f, chunks[1], Color::Reset);

        // Save back drawer scroll and restore previous state
        self.ui.drawer_scroll = self.ui.selected_agent_output_scroll;
        self.ui.drawer_content_lines = self.ui.selected_agent_output_lines;
        self.ui.selected_agent_output_scroll = prev_scroll;
        self.agents.selected_inline_agent = prev_selected;
    }

    /// Save the first visible line index before a drawer resize.
    pub fn save_scroll_anchor(&mut self) {
        let chat_height = self.layout.last_chat_rect.height as usize;
        let max_scroll = self.ui.max_scroll.get();
        let safe_scroll = self.ui.scroll_offset.min(max_scroll);
        let total = self.layout.chat_total_lines;
        let first_visible = if total > chat_height {
            total
                .saturating_sub(chat_height)
                .saturating_sub(safe_scroll)
        } else {
            0
        };
        self.ui.scroll_anchor_line = Some(first_visible);
    }

    /// Restore scroll position after a drawer resize.
    pub fn restore_scroll_anchor(&mut self) {
        if let Some(anchor) = self.ui.scroll_anchor_line.take() {
            let chat_height = self.layout.last_chat_rect.height as usize;
            let total = self.layout.chat_total_lines;
            let max_scroll = total.saturating_sub(chat_height);
            let new_offset = total.saturating_sub(chat_height).saturating_sub(anchor);
            self.ui.scroll_offset = new_offset.min(max_scroll);
        }
    }
}
