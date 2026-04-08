//! Event Handlers Module

pub mod agent;
pub mod ipc;
pub mod keyboard;

use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};
use tracing::error;

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
        Ok(())
    }

    /// Handle mouse event
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
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

                // Check Activity Panel click
                if let Some(ref activity_rect) = self.layout.last_activity_rect {
                    if x >= activity_rect.x
                        && x < activity_rect.x + activity_rect.width
                        && y >= activity_rect.y
                        && y < activity_rect.y + activity_rect.height
                    {
                        // Calculate click position relative to content area (summary_area already has padding built in)
                        let content_y = (y - activity_rect.y) as usize;
                        // Adjust for scroll offset
                        let adjusted_y = content_y + self.ui.activity_scroll_offset;

                        // Match against activity_agent_y_positions
                        // Index 0 in activity_agent_y_positions is now "Main Session" (usize::MAX)
                        for (idx, &agent_y) in
                            self.layout.activity_agent_y_positions.iter().enumerate()
                        {
                            if adjusted_y == agent_y {
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
                        for (idx, &diff_y) in
                            self.layout.activity_diff_y_positions.iter().enumerate()
                        {
                            if adjusted_y == diff_y && idx < self.git_changes.len() {
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
                        // Calculate position relative to tab bar (tabs start after "Tabs ")
                        let rel_x = (x - tab_rect.x) as usize;
                        // Each tab is roughly " N Name " format
                        // "Tabs " = 5 chars, then tabs at ~8 chars each
                        if rel_x >= 5 {
                            let tab_index = (rel_x - 5) / 8;
                            match tab_index {
                                0 => self.selected_right_pane_tab = RightPaneTab::Agent,
                                1 => self.selected_right_pane_tab = RightPaneTab::Diff,
                                2 => self.selected_right_pane_tab = RightPaneTab::Batch,
                                3 => self.selected_right_pane_tab = RightPaneTab::Trust,
                                _ => {}
                            }
                            self.ui.selected_agent_output_scroll = 0;
                            return Ok(());
                        }
                    }
                }

                // Check Mode Bar click
                if let Some(ref mode_rect) = self.layout.last_mode_bar_rect {
                    if x >= mode_rect.x
                        && x < mode_rect.x + mode_rect.width
                        && y >= mode_rect.y
                        && y < mode_rect.y + mode_rect.height
                    {
                        // Calculate position relative to mode bar
                        let rel_x = (x - mode_rect.x) as usize;
                        // "Mode " = 5 chars, then each mode is " Mode " = 6 chars
                        if rel_x >= 5 {
                            let mode_index = (rel_x - 5) / 6;
                            let modes = crate::app::state::FocusMode::ALL;
                            if mode_index < modes.len() {
                                self.ui.focus_mode = modes[mode_index];
                                return Ok(());
                            }
                        }
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
