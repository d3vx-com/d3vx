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

    /// Rehydrate the `recent_tools` activity panel from the persisted
    /// [`tool_executions`](crate::store::ToolExecutionStore) table when
    /// there is nothing live to show.
    ///
    /// The panel is normally fed by live IPC events from the attached
    /// agent. On TUI restart — or in standalone / inspect-only modes —
    /// there are no events; without this, the panel sits empty even
    /// when the DB holds the full tool history for the session.
    ///
    /// Guard conditions:
    /// * `recent_tools` is already non-empty → IPC is live, skip.
    /// * No DB handle or no session ID → nothing to query, skip.
    /// * DB read fails → log at debug and leave the panel empty.
    pub fn poll_tool_executions(&mut self) -> Result<()> {
        if !self.tools.recent_tools.is_empty() {
            return Ok(());
        }

        let Some(session_id) = self.session.session_id.clone() else {
            return Ok(());
        };
        let Some(db_handle) = self.db.clone() else {
            return Ok(());
        };

        let records = {
            let db = db_handle.lock();
            let store = crate::store::ToolExecutionStore::new(&db);
            match store.list_for_session(&session_id) {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(
                        error = %e,
                        session = %session_id,
                        "tool audit rehydration skipped"
                    );
                    return Ok(());
                }
            }
        };

        const MAX_REHYDRATE: usize = 20;
        self.tools
            .recent_tools
            .extend(records_to_tool_states(records, MAX_REHYDRATE));

        Ok(())
    }
}

/// Convert audit records into UI-side tool-execution states.
///
/// Pure helper so the tricky bits (JSON re-parse, duration clamping,
/// most-recent-N windowing) are testable without a full `App`.
pub(super) fn records_to_tool_states(
    records: Vec<crate::store::ToolExecutionRecord>,
    limit: usize,
) -> Vec<crate::app::state::ToolExecutionState> {
    let skip = records.len().saturating_sub(limit);
    records
        .into_iter()
        .skip(skip)
        .map(|record| {
            let input = serde_json::from_str::<serde_json::Value>(&record.tool_input)
                .unwrap_or(serde_json::Value::Null);
            // `duration_ms` is `i64` in storage but logically unsigned;
            // clamp to avoid surprising wrap on malformed rows.
            let elapsed_ms = record.duration_ms.unwrap_or(0).max(0) as u64;
            crate::app::state::ToolExecutionState {
                // Prefix rehydrated IDs so they can't collide with
                // live IPC tool IDs (UUIDs) that arrive after this.
                id: format!("audit-{}", record.id),
                name: record.tool_name,
                input,
                start_time: std::time::Instant::now(),
                is_executing: false,
                output: record.tool_result,
                is_error: record.is_error,
                elapsed_ms,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::records_to_tool_states;
    use crate::store::ToolExecutionRecord;

    fn rec(id: i64, name: &str, is_err: bool, dur: Option<i64>) -> ToolExecutionRecord {
        ToolExecutionRecord {
            id,
            session_id: "s1".to_string(),
            tool_name: name.to_string(),
            tool_input: r#"{"path":"foo"}"#.to_string(),
            tool_result: Some("result".to_string()),
            is_error: is_err,
            duration_ms: dur,
            created_at: "2026-04-19T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn records_to_tool_states_preserves_order_within_limit() {
        let records = vec![
            rec(1, "Read", false, Some(10)),
            rec(2, "Bash", false, Some(20)),
            rec(3, "Edit", false, Some(30)),
        ];
        let states = records_to_tool_states(records, 20);
        assert_eq!(states.len(), 3);
        assert_eq!(states[0].name, "Read");
        assert_eq!(states[1].name, "Bash");
        assert_eq!(states[2].name, "Edit");
    }

    #[test]
    fn records_to_tool_states_keeps_most_recent_when_over_limit() {
        // Input is oldest-first; we keep the last N to show the most
        // recent activity (the panel is a "what just happened" view).
        let records: Vec<_> = (0..30)
            .map(|i| rec(i as i64, &format!("Tool{i}"), false, Some(i as i64)))
            .collect();
        let states = records_to_tool_states(records, 5);
        assert_eq!(states.len(), 5);
        assert_eq!(states[0].name, "Tool25");
        assert_eq!(states[4].name, "Tool29");
    }

    #[test]
    fn records_to_tool_states_prefixes_ids_to_avoid_collision() {
        let records = vec![rec(42, "Read", false, Some(5))];
        let states = records_to_tool_states(records, 20);
        assert_eq!(states[0].id, "audit-42");
    }

    #[test]
    fn records_to_tool_states_marks_rehydrated_as_not_executing() {
        let records = vec![rec(1, "Read", false, Some(10))];
        let states = records_to_tool_states(records, 20);
        assert!(!states[0].is_executing);
    }

    #[test]
    fn records_to_tool_states_handles_missing_duration() {
        let records = vec![rec(1, "Read", false, None)];
        let states = records_to_tool_states(records, 20);
        assert_eq!(states[0].elapsed_ms, 0);
    }

    #[test]
    fn records_to_tool_states_clamps_negative_duration_to_zero() {
        let records = vec![rec(1, "Read", false, Some(-5))];
        let states = records_to_tool_states(records, 20);
        assert_eq!(states[0].elapsed_ms, 0);
    }

    #[test]
    fn records_to_tool_states_round_trips_error_flag() {
        let records = vec![rec(1, "Bash", true, Some(10))];
        let states = records_to_tool_states(records, 20);
        assert!(states[0].is_error);
    }

    #[test]
    fn records_to_tool_states_parses_json_input() {
        let mut r = rec(1, "Read", false, Some(10));
        r.tool_input = r#"{"file_path":"/tmp/x","replace_all":true}"#.to_string();
        let states = records_to_tool_states(vec![r], 20);
        assert_eq!(states[0].input["file_path"], "/tmp/x");
        assert_eq!(states[0].input["replace_all"], true);
    }

    #[test]
    fn records_to_tool_states_falls_back_to_null_on_bad_json() {
        let mut r = rec(1, "Read", false, Some(10));
        r.tool_input = "not valid json".to_string();
        let states = records_to_tool_states(vec![r], 20);
        assert!(states[0].input.is_null());
    }

    #[test]
    fn records_to_tool_states_empty_input_yields_empty_output() {
        let states = records_to_tool_states(Vec::new(), 20);
        assert!(states.is_empty());
    }
}
