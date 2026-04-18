//! Event Handlers Module

pub mod agent;
pub mod ipc;
pub mod keyboard;

#[cfg(test)]
mod ipc_tests;

use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};
use tracing::error;

use crate::app::state::DrawerHeight;
use crate::app::{App, RightPaneTab};
use crate::event::Event;

impl App {
    /// Main event entry point
    pub async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                // Catch errors from key handling and log instead of failing
                if let Err(e) = self.handle_key_event(key).await {
                    error!(?e, "Error handling key event");
                }
            }
            Event::Mouse(mouse) => self.handle_mouse_event(mouse)?,
            Event::Paste(text) => self.handle_paste(text)?,
            Event::Resize(_, _) => {}
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Tick => self.handle_tick()?,
            Event::Ipc(ipc_event) => self.handle_ipc_event(ipc_event).await?,
            Event::ShellResult {
                cmd,
                output,
                exit_code,
            } => self.handle_shell_result(cmd, output, exit_code)?,
            Event::SaveSession => {
                if let Err(e) = self.save_current_session().await {
                    error!(?e, "Failed to save session");
                }
            }
            Event::ToolCompleted {
                id,
                output,
                is_error,
                elapsed_ms,
            } => {
                self.complete_tool_call(&id, output, is_error, elapsed_ms);
            }
            Event::Agent(agent_event) => {
                self.handle_agent_event(agent_event).await?;
            }
            Event::AgentInWorkspace(id, agent_event) => {
                self.handle_workspace_agent_event(&id, agent_event).await?;
            }
            Event::SendMessage(text) => {
                self.ui.mode = crate::app::AppMode::Chat;
                self.execute_message(text)?;
            }
            Event::RunSynthesis => {
                // Only trigger synthesis if the agent is truly idle:
                // - not currently thinking/running
                // - no agent loop actively processing
                // This prevents duplicate RunSynthesis events from restarting
                // the loop after it already processed child results.
                let has_active_loop =
                    self.agents.agent_loop.is_some() && self.agents.running_parallel_agents == 0;
                if !self.session.thinking.is_thinking && has_active_loop {
                    self.run_agent_loop();
                }
            }
            Event::SpawnParallel(event) => {
                if let Err(e) = self.handle_spawn_parallel_event(event).await {
                    error!(?e, "Error handling spawn parallel event");
                }
            }
            Event::InboxMessage {
                to_agent,
                from_agent,
                message,
            } => {
                self.handle_inbox_message(&to_agent, &from_agent, &message)
                    .await?;
            }
            Event::SwarmRelay { from, to: _, body } => {
                self.add_system_message(&format!("[swarm] {}: {}", from, body));
            }
            Event::Error(msg) => {
                self.add_system_message(&format!("Error: {}", msg));
            }
        }
        self.needs_redraw = true;
        Ok(())
    }

    /// Handle mouse event
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                // Check drawer scroll first
                if let Some(drawer_rect) = self.layout.last_drawer_rect {
                    if mouse.column >= drawer_rect.x
                        && mouse.column < drawer_rect.x + drawer_rect.width
                        && mouse.row >= drawer_rect.y
                        && mouse.row < drawer_rect.y + drawer_rect.height
                    {
                        self.ui.drawer_scroll = self.ui.drawer_scroll.saturating_sub(3);
                        return Ok(());
                    }
                }
                if let Some(detail_rect) = self.layout.last_agent_detail_rect {
                    if mouse.column >= detail_rect.x
                        && mouse.column < detail_rect.x + detail_rect.width
                        && mouse.row >= detail_rect.y
                        && mouse.row < detail_rect.y + detail_rect.height
                    {
                        self.ui.selected_agent_output_scroll =
                            self.ui.selected_agent_output_scroll.saturating_sub(3);
                        return Ok(());
                    }
                }
                // Check if mouse is over activity panel
                if let Some(ref activity_rect) = self.layout.last_activity_rect {
                    if mouse.column >= activity_rect.x
                        && mouse.column < activity_rect.x + activity_rect.width
                        && mouse.row >= activity_rect.y
                        && mouse.row < activity_rect.y + activity_rect.height
                    {
                        // Scroll activity panel
                        let max_activity_scroll = self
                            .ui
                            .activity_content_lines
                            .saturating_sub(activity_rect.height as usize);
                        self.ui.activity_scroll_offset = self
                            .ui
                            .activity_scroll_offset
                            .saturating_add(3)
                            .min(max_activity_scroll);
                        return Ok(());
                    }
                }
                // Otherwise scroll main chat
                self.ui.scroll_offset = self
                    .ui
                    .scroll_offset
                    .saturating_add(1)
                    .min(self.ui.max_scroll.get());
            }
            MouseEventKind::ScrollDown => {
                // Check drawer scroll first
                if let Some(drawer_rect) = self.layout.last_drawer_rect {
                    if mouse.column >= drawer_rect.x
                        && mouse.column < drawer_rect.x + drawer_rect.width
                        && mouse.row >= drawer_rect.y
                        && mouse.row < drawer_rect.y + drawer_rect.height
                    {
                        let max_scroll = self
                            .ui
                            .drawer_content_lines
                            .saturating_sub(drawer_rect.height.saturating_sub(2) as usize);
                        self.ui.drawer_scroll = (self.ui.drawer_scroll + 3).min(max_scroll);
                        return Ok(());
                    }
                }
                if let Some(detail_rect) = self.layout.last_agent_detail_rect {
                    if mouse.column >= detail_rect.x
                        && mouse.column < detail_rect.x + detail_rect.width
                        && mouse.row >= detail_rect.y
                        && mouse.row < detail_rect.y + detail_rect.height
                    {
                        let max_detail_scroll = self
                            .ui
                            .selected_agent_output_lines
                            .saturating_sub(detail_rect.height as usize);
                        self.ui.selected_agent_output_scroll =
                            (self.ui.selected_agent_output_scroll + 3).min(max_detail_scroll);
                        return Ok(());
                    }
                }
                // Check if mouse is over activity panel
                if let Some(ref activity_rect) = self.layout.last_activity_rect {
                    if mouse.column >= activity_rect.x
                        && mouse.column < activity_rect.x + activity_rect.width
                        && mouse.row >= activity_rect.y
                        && mouse.row < activity_rect.y + activity_rect.height
                    {
                        // Scroll activity panel
                        self.ui.activity_scroll_offset =
                            self.ui.activity_scroll_offset.saturating_sub(3);
                        return Ok(());
                    }
                }
                // Otherwise scroll main chat
                self.ui.scroll_offset = self.ui.scroll_offset.saturating_sub(1);
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                // Check Agent Strip click — pill or empty area
                if let Some(strip_rect) = self.layout.last_strip_rect {
                    if x >= strip_rect.x
                        && x < strip_rect.x + strip_rect.width
                        && y >= strip_rect.y
                        && y < strip_rect.y + strip_rect.height
                    {
                        // Check if clicked on a specific pill
                        let mut clicked_pill = None;
                        for &(start_col, end_col, agent_idx) in &self.layout.strip_pill_positions {
                            if x >= start_col && x < end_col {
                                clicked_pill = Some(agent_idx);
                                break;
                            }
                        }

                        if let Some(agent_idx) = clicked_pill {
                            // Select this agent and open drawer
                            if agent_idx < self.agents.inline_agents.len() {
                                self.agents.selected_inline_agent = Some(agent_idx);
                                self.ui.drawer_agent_id =
                                    Some(self.agents.inline_agents[agent_idx].id.clone());
                                self.save_scroll_anchor();
                                self.ui.drawer_height = DrawerHeight::Percent30;
                                self.ui.drawer_scroll = 0;
                            }
                        } else {
                            // Clicked empty strip area — toggle expanded
                            self.ui.strip_expanded = !self.ui.strip_expanded;
                        }
                        return Ok(());
                    }
                }

                // Check Activity Panel click
                if let Some(ref activity_rect) = self.layout.last_activity_rect {
                    if x >= activity_rect.x
                        && x < activity_rect.x + activity_rect.width
                        && y >= activity_rect.y
                        && y < activity_rect.y + activity_rect.height
                    {
                        // Calculate click position relative to content area
                        let content_y = (y - activity_rect.y) as usize;
                        // Adjust for scroll offset
                        let adjusted_y = content_y + self.ui.activity_scroll_offset;

                        // Range-based agent row matching:
                        // Each stored Y marks the *start* of that row. A click hits
                        // a row if it's on or after that row's Y, but before the
                        // next row's Y (or the end of the list).
                        let agent_positions = &self.layout.activity_agent_y_positions;
                        for (idx, &start_y) in agent_positions.iter().enumerate() {
                            let next_y =
                                agent_positions.get(idx + 1).copied().unwrap_or(usize::MAX);
                            if adjusted_y >= start_y && adjusted_y < next_y {
                                if idx == 0 {
                                    self.agents.selected_inline_agent = Some(usize::MAX);
                                } else if idx - 1 < self.agents.inline_agents.len() {
                                    self.agents.selected_inline_agent = Some(idx - 1);
                                }
                                self.ui.selected_agent_output_scroll = 0;
                                return Ok(());
                            }
                        }

                        let mut found_diff = false;
                        let diff_positions = &self.layout.activity_diff_y_positions;
                        for (idx, &start_y) in diff_positions.iter().enumerate() {
                            let next_y = diff_positions.get(idx + 1).copied().unwrap_or(usize::MAX);
                            if adjusted_y >= start_y
                                && adjusted_y < next_y
                                && idx < self.git_changes.len()
                            {
                                self.select_git_change(idx);
                                found_diff = true;
                                break;
                            }
                        }

                        // Only reset scroll if didn't click on any actionable row
                        if !found_diff {
                            self.ui.activity_scroll_offset = 0;
                        }
                    }
                }

                // Check Tab Bar click
                if let Some(ref tab_rect) = self.layout.last_tab_bar_rect {
                    if x >= tab_rect.x
                        && x < tab_rect.x + tab_rect.width
                        && y >= tab_rect.y
                        && y < tab_rect.y + tab_rect.height
                    {
                        let rel_x = (x - tab_rect.x) as usize;
                        // Actual rendered tab widths: " N:Label " with "│" between tabs
                        let tab_labels = ["1:Agent", "2:Diff", "3:Batch", "4:Readiness"];
                        let tab_tabs = [
                            RightPaneTab::Agent,
                            RightPaneTab::Diff,
                            RightPaneTab::Batch,
                            RightPaneTab::Trust,
                        ];
                        let mut offset = 0usize;
                        for (i, label) in tab_labels.iter().enumerate() {
                            let tab_width = label.len() + 2; // " " + label + " "
                            if rel_x >= offset && rel_x < offset + tab_width {
                                self.selected_right_pane_tab = tab_tabs[i];
                                self.ui.selected_agent_output_scroll = 0;
                                return Ok(());
                            }
                            offset += tab_width;
                            // Skip separator "│" between tabs
                            if i < tab_labels.len() - 1 {
                                offset += 1;
                            }
                        }
                    }
                }

                // Check Mode Bar click — click on the badge to cycle modes
                if let Some(ref mode_rect) = self.layout.last_mode_bar_rect {
                    if x >= mode_rect.x
                        && x < mode_rect.x + mode_rect.width
                        && y >= mode_rect.y
                        && y < mode_rect.y + mode_rect.height
                    {
                        self.ui.focus_mode = self.ui.focus_mode.cycle(false);
                        return Ok(());
                    }
                }

                // Check legacy sidebar click (Board/List modes)
                if self.ui.right_sidebar_visible
                    && x >= self.layout.last_right_sidebar_rect.x
                    && x < self.layout.last_right_sidebar_rect.x
                        + self.layout.last_right_sidebar_rect.width
                    && y >= self.layout.last_right_sidebar_rect.y
                    && y < self.layout.last_right_sidebar_rect.y
                        + self.layout.last_right_sidebar_rect.height
                {
                    let sidebar_inner_y =
                        y.saturating_sub(self.layout.last_right_sidebar_rect.y + 1);
                    let row = sidebar_inner_y as usize;

                    // Check for agent row click (expand/collapse)
                    if let Some(&agent_idx) = self.layout.sidebar_agent_rows.get(row) {
                        if agent_idx != usize::MAX && agent_idx < self.agents.inline_agents.len() {
                            self.select_inline_agent(agent_idx);
                        }
                    }
                    // Check for workspace row click
                    else if let Some(Some(workspace_index)) =
                        self.layout.left_sidebar_workspace_rows.get(row)
                    {
                        if *workspace_index < self.workspaces.len() {
                            self.switch_workspace(*workspace_index)?;
                        }
                    }
                }

                // Check Chat Area click for agent rows
                if x >= self.layout.last_chat_rect.x
                    && x < self.layout.last_chat_rect.x + self.layout.last_chat_rect.width
                    && y >= self.layout.last_chat_rect.y
                    && y < self.layout.last_chat_rect.y + self.layout.last_chat_rect.height
                {
                    let chat_height = self.layout.last_chat_rect.height as usize;
                    let click_y_in_chat = (y - self.layout.last_chat_rect.y) as usize;

                    // Calculate which line index was clicked based on scroll
                    let max_scroll = self.layout.chat_total_lines.saturating_sub(chat_height);
                    let safe_scroll = self.ui.scroll_offset.min(max_scroll);
                    let visible_start_line = if self.layout.chat_total_lines > chat_height {
                        self.layout.chat_total_lines - chat_height - safe_scroll
                    } else {
                        0
                    };
                    let clicked_line = visible_start_line + click_y_in_chat;

                    // Check if clicked line matches any agent row
                    for &(line_idx, agent_idx) in &self.layout.chat_agent_y_positions {
                        if line_idx as usize == clicked_line
                            && agent_idx < self.agents.inline_agents.len()
                        {
                            self.select_inline_agent(agent_idx);
                            return Ok(());
                        }
                    }

                    // Not on an agent row - focus chat
                    self.ui.mode = crate::app::AppMode::Chat;
                }

                // Check Main Area click (focus chat)
                if x >= self.layout.last_input_rect.x
                    && x < self.layout.last_input_rect.x + self.layout.last_input_rect.width
                    && y >= self.layout.last_input_rect.y
                    && y < self.layout.last_input_rect.y + self.layout.last_input_rect.height
                {
                    self.ui.mode = crate::app::AppMode::Chat;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle tick
    pub fn handle_tick(&mut self) -> Result<()> {
        Ok(())
    }
}
