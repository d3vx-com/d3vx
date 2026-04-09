//! Application State Update Loop
//!
//! Handles periodic refresh of git status, workspaces, orchestrator state,
//! notifications, and animation frames.

use anyhow::Result;
use std::time::{Duration, Instant};

use crate::app::state::WorkspaceType;
use crate::app::App;
use crate::pipeline::Phase;

impl App {
    /// Add a transient notification
    pub fn add_notification(
        &mut self,
        message: impl Into<String>,
        n_type: crate::app::state::NotificationType,
    ) {
        self.notifications.push(crate::app::state::Notification {
            message: message.into(),
            notification_type: n_type,
            timestamp: std::time::Instant::now(),
            duration: std::time::Duration::from_secs(5),
        });
    }

    /// Check if there is any background activity (sub-agents or Vex tasks)
    pub fn has_background_activity(&self) -> bool {
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let has_subagents = rt
                .block_on(self.subagents.list())
                .iter()
                .any(|a| a.status == crate::agent::SubAgentStatus::Running);
            let has_bg_tasks = !self.background_active_tasks.is_empty();
            has_subagents || has_bg_tasks
        })
    }

    /// Update application state (unbound from event loop)
    pub fn update(&mut self) {
        // Prune expired notifications
        let now = std::time::Instant::now();
        self.notifications
            .retain(|n| now.duration_since(n.timestamp) < n.duration);

        // Handle animation
        if self.last_update.elapsed() >= Duration::from_millis(100) {
            self.animation_frame = self.animation_frame.wrapping_add(1);
            self.last_update = Instant::now();
        }

        // Refresh git status periodically
        if self.last_git_refresh.elapsed() >= Duration::from_secs(5) {
            let _ = self.refresh_git_status();
            self.last_git_refresh = Instant::now();
        }

        // Refresh workspaces more frequently if we have active pipeline tasks
        let refresh_interval = if self.autonomous_mode
            || !self
                .workspaces
                .iter()
                .all(|w| w.status == crate::app::state::WorkspaceStatus::Idle)
        {
            Duration::from_secs(2)
        } else {
            Duration::from_secs(10)
        };

        if self.last_workspace_refresh.elapsed() >= refresh_interval {
            let _ = self.refresh_workspaces();
            let _ = self.refresh_task_views();
            self.last_workspace_refresh = Instant::now();
        }

        // Poll for vex agent updates (every 500ms)
        if self.last_orchestrator_refresh.elapsed() >= Duration::from_millis(500) {
            // Update orchestrator stats
            self.background_active_tasks = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.orchestrator.active_tasks_list())
            });
            self.background_queue_stats = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.orchestrator.queue_stats())
            });
            self.background_worker_stats = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.orchestrator.worker_pool_stats())
            });

            // Poll vex agents and add to inline_agents
            if let Some(ref db) = self.db {
                let project_path = self.cwd.as_deref().unwrap_or(".");
                let vex_agents = crate::app::vex_agent_poller::poll_vex_agents(db, project_path);

                // Update or add vex agents
                for vex_agent in vex_agents {
                    let existing = self
                        .agents
                        .inline_agents
                        .iter()
                        .position(|a| a.id == vex_agent.id);

                    if let Some(idx) = existing {
                        // Update existing vex agent
                        self.agents.inline_agents[idx] = vex_agent;
                    } else {
                        // Add new vex agent
                        self.agents.inline_agents.push(vex_agent);
                    }
                }
            }

            self.last_orchestrator_refresh = Instant::now();

            // Cache subagent count for render (avoids block_in_place in render path)
            self.cached_subagent_count = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.subagents.list())
                    .iter()
                    .filter(|a| a.status == crate::agent::SubAgentStatus::Running)
                    .count()
            });
        }

        // Update thinking state if orchestrator is active
        if !self.background_active_tasks.is_empty() {
            if !self.session.thinking.is_thinking {
                self.session.thinking.is_thinking = true;
                self.session.thinking.text = "Background Task Active".to_string();
                self.session.thinking_start = Some(Instant::now());
            }
        } else if self.session.thinking.text == "Background Task Active"
            && self.agents.streaming_message.is_empty()
        {
            self.session.thinking = crate::ipc::ThinkingState::default();
            self.session.thinking_start = None;
        }

        // If in Vex workspace, sync thinking phase from the task
        let current_ws_idx = self.workspace_selected_index;
        if let Some(ws) = self.workspaces.get(current_ws_idx) {
            if ws.workspace_type == WorkspaceType::Satellite {
                if let Some(phase_str) = &ws.phase {
                    if let Some(phase) = Phase::from_str_ignore_case(phase_str) {
                        self.session.thinking.phase = match phase {
                            Phase::Research => crate::ipc::ThinkingPhase::Research,
                            Phase::Ideation => crate::ipc::ThinkingPhase::Research,
                            Phase::Plan => crate::ipc::ThinkingPhase::Plan,
                            Phase::Draft => crate::ipc::ThinkingPhase::Draft,
                            Phase::Implement => crate::ipc::ThinkingPhase::Implement,
                            Phase::Review => crate::ipc::ThinkingPhase::Review,
                            Phase::Docs => crate::ipc::ThinkingPhase::Docs,
                        };
                    }
                }
            }
        }

        // Standalone tool updates
        let _ = self.poll_tool_executions();

        // Request redraw if state changed
        self.needs_redraw = true;
    }

    /// Poll for completed tools (placeholder)
    pub fn poll_tool_executions(&mut self) -> Result<()> {
        Ok(())
    }
}
