//! Chat input keyboard handling — control-key combos and mode switches

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::state::RightPaneTab;
use crate::app::{App, AppMode};

impl App {
    /// Handle chat input key events (delegates to navigation for text keys)
    pub(crate) fn handle_input_key(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            // Quit or stop conversation (Ctrl+C)
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                let now = std::time::Instant::now();

                // Check if this is a rapid second Ctrl+C (within 500ms)
                let is_double_press = self
                    .last_ctrl_c_time
                    .map(|t| now.duration_since(t).as_millis() < 500)
                    .unwrap_or(false);

                // Reset counter if not a rapid press
                if !is_double_press {
                    self.ctrl_c_count = 0;
                }

                self.last_ctrl_c_time = Some(now);
                self.ctrl_c_count += 1;

                // Force quit on double Ctrl+C or if already quitting
                if self.ctrl_c_count >= 2 || self.should_quit {
                    self.should_quit = true;
                    self.ctrl_c_count = 0;
                    return Ok(());
                }

                // If currently thinking or streaming, stop the conversation
                if self.session.thinking.is_thinking
                    || !self.agents.streaming_message.is_empty()
                    || self.agents.is_connected
                {
                    self.stop_conversation()?;
                } else if !self.ui.input_buffer.is_empty() {
                    // Empty the input buffer instead of quitting directly
                    self.ui.input_buffer.clear();
                    self.ui.cursor_position = 0;
                    self.clear_mention_picker();
                } else {
                    // First Ctrl+C when idle - set should_quit and show hint
                    self.should_quit = true;
                    self.add_system_message("Press Ctrl+C again to force quit");
                }
            }

            // Toggle tools expand in activity panel (Ctrl+O)
            // Also toggles tool expansion for selected inline agent
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.tools.activity_tools_expanded = !self.tools.activity_tools_expanded;

                // If an inline agent is selected, toggle its tool expansion too
                if let Some(idx) = self.agents.selected_inline_agent {
                    if let Some(agent) = self.agents.inline_agents.get_mut(idx) {
                        agent.show_tools = !agent.show_tools;
                    }
                }
            }

            // Inline agent navigation (Ctrl+Up/Down to navigate, Enter to expand/collapse)
            (KeyCode::Up, KeyModifiers::CONTROL) => {
                if !self.agents.inline_agents.is_empty() {
                    let new_index = match self.agents.selected_inline_agent {
                        None => self.agents.inline_agents.len() - 1,
                        Some(i) if i == 0 => self.agents.inline_agents.len() - 1,
                        Some(i) => i - 1,
                    };
                    self.select_inline_agent(new_index);
                }
            }
            (KeyCode::Down, KeyModifiers::CONTROL) => {
                if !self.agents.inline_agents.is_empty() {
                    let new_index = match self.agents.selected_inline_agent {
                        None => 0,
                        Some(i) if i >= self.agents.inline_agents.len() - 1 => 0,
                        Some(i) => i + 1,
                    };
                    self.select_inline_agent(new_index);
                }
            }
            (KeyCode::Enter, KeyModifiers::CONTROL)
            | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                let agent_index = if let Some(idx) = self.agents.selected_inline_agent {
                    Some(idx)
                } else if !self.agents.inline_agents.is_empty() {
                    Some(0)
                } else {
                    None
                };
                if let Some(index) = agent_index {
                    self.select_inline_agent(index);
                }
            }

            // Scroll selected agent transcript in the right pane
            (KeyCode::PageUp, KeyModifiers::ALT) => {
                self.ui.selected_agent_output_scroll =
                    self.ui.selected_agent_output_scroll.saturating_sub(3);
            }
            (KeyCode::PageDown, KeyModifiers::ALT) => {
                let max_scroll = self.ui.selected_agent_output_lines.saturating_sub(
                    self.layout
                        .last_agent_detail_rect
                        .map(|r| r.height as usize)
                        .unwrap_or(0),
                );
                self.ui.selected_agent_output_scroll =
                    (self.ui.selected_agent_output_scroll + 3).min(max_scroll);
            }

            // Toggle unified sidebar (Navigator / Inspector)
            (KeyCode::Char('l'), KeyModifiers::CONTROL)
            | (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.ui.right_sidebar_visible = !self.ui.right_sidebar_visible;
            }

            // Toggle agent monitor inside the sidebar
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.ui.agent_monitor_pinned = !self.ui.agent_monitor_pinned;
                self.ui.right_sidebar_visible = true;
            }

            // Show sidebar (with navigator focus)
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.ui.right_sidebar_visible = true;
                self.ui.agent_monitor_pinned = false;
            }

            // Global Workspace Switching (Alt+Left/Right)
            (KeyCode::Left, KeyModifiers::ALT) => {
                if self.workspace_selected_index > 0 {
                    self.switch_workspace(self.workspace_selected_index - 1)?;
                }
            }
            (KeyCode::Right, KeyModifiers::ALT) => {
                if self.workspace_selected_index + 1 < self.workspaces.len() {
                    self.switch_workspace(self.workspace_selected_index + 1)?;
                }
            }

            // Power Mode Toggle (Vibe Mode escape hatch)
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.ui.power_mode = !self.ui.power_mode;
                self.ui.right_sidebar_visible = self.ui.power_mode;
                self.ui.verbose = self.ui.power_mode;
                self.add_notification(
                    if self.ui.power_mode {
                        "Power Mode: ON (Advanced telemetry enabled)"
                    } else {
                        "Vibe Mode: ON (Telemetry hidden)"
                    }
                    .to_string(),
                    crate::app::state::NotificationType::Info,
                );
            }

            // Toggle diff preview (if there's diff content)
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.diff_view.is_none() {
                    self.diff_view = self.diff_preview.clone();
                }
                if self.diff_view.is_some() {
                    self.ui.mode = if self.ui.mode == AppMode::DiffPreview {
                        AppMode::Chat
                    } else {
                        AppMode::DiffPreview
                    };
                }
            }

            // Right pane tab switching
            (KeyCode::Char('1'), KeyModifiers::CONTROL) => {
                self.selected_right_pane_tab = RightPaneTab::Agent;
            }
            (KeyCode::Char('2'), KeyModifiers::CONTROL) => {
                self.selected_right_pane_tab = RightPaneTab::Diff;
            }
            (KeyCode::Char('3'), KeyModifiers::CONTROL) => {
                self.selected_right_pane_tab = RightPaneTab::Batch;
            }

            // Diff-file cycling in the operator pane
            (KeyCode::Left, KeyModifiers::CONTROL) => {
                self.cycle_git_change(-1);
            }
            (KeyCode::Right, KeyModifiers::CONTROL) => {
                self.cycle_git_change(1);
            }

            // Focus-mode cycling
            (KeyCode::Tab, mods) if mods.contains(KeyModifiers::CONTROL) => {
                let reverse = mods.contains(KeyModifiers::SHIFT);
                self.ui.focus_mode = self.ui.focus_mode.cycle(reverse);
                self.add_notification(
                    format!("Focus mode: {}", self.ui.focus_mode.label()),
                    crate::app::state::NotificationType::Info,
                );
            }

            // Clear input
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.ui.input_buffer.clear();
                self.ui.cursor_position = 0;
                self.clear_mention_picker();
            }

            // Pop from queue
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                self.session.message_queue.pop();
            }

            // Delegate remaining keys to navigation handler
            _ => self.handle_input_navigation_key(key)?,
        }
        Ok(())
    }
}
