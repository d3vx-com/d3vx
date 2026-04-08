//! Dashboard Bridge
//!
//! Forwards AgentEvent from the agent loop to the Dashboard's SSE broadcast
//! channel so browsers can watch agent activity in real-time.

use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::agent::AgentEvent;
use crate::pipeline::dashboard::{Dashboard, DashboardEvent};

/// Bridges agent events to the dashboard SSE stream.
///
/// Subscribes to an AgentLoop's broadcast channel and translates each event
/// into a DashboardEvent that gets streamed to connected browsers.
pub struct DashboardBridge;

impl DashboardBridge {
    /// Spawn a background task that forwards agent events to the dashboard.
    ///
    /// Returns immediately; the forwarding runs in the background until the
    /// agent event channel closes.
    pub fn spawn(
        dashboard: Dashboard,
        agent_rx: broadcast::Receiver<AgentEvent>,
        session_id: String,
    ) {
        tokio::spawn(async move {
            Self::forward_loop(dashboard, agent_rx, &session_id).await;
        });
    }

    /// Main forwarding loop. Runs until the agent channel closes.
    async fn forward_loop(
        dashboard: Dashboard,
        mut rx: broadcast::Receiver<AgentEvent>,
        session_id: &str,
    ) {
        debug!(session_id, "Dashboard bridge started for session");

        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(dash_event) = Self::to_dashboard_event(event, session_id) {
                        dashboard.broadcast(dash_event);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!(session_id, "Dashboard bridge: agent channel closed");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(session_id, lagged = n, "Dashboard bridge: lagged, skipping");
                    continue;
                }
            }
        }
    }

    /// Translate an AgentEvent into a DashboardEvent.
    ///
    /// Returns None for events that aren't useful for the dashboard UI.
    fn to_dashboard_event(event: AgentEvent, session_id: &str) -> Option<DashboardEvent> {
        match event {
            AgentEvent::Start { .. } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: "started".to_string(),
            }),

            AgentEvent::Thinking { text } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("thinking: {}", truncate(&text, 200)),
            }),

            AgentEvent::Text { text } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("response: {}", truncate(&text, 200)),
            }),

            AgentEvent::ToolStart { name, .. } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("tool: {}", name),
            }),

            AgentEvent::ToolEnd { name, is_error, .. } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: if is_error {
                    format!("tool_error: {}", name)
                } else {
                    format!("tool_done: {}", name)
                },
            }),

            AgentEvent::Error { error } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("error: {}", truncate(&error, 200)),
            }),

            AgentEvent::Done {
                iterations,
                tool_calls,
                total_usage,
                ..
            } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!(
                    "done: {} iterations, {} tools, {} tokens",
                    iterations,
                    tool_calls,
                    total_usage.total()
                ),
            }),

            AgentEvent::IterationEnd {
                iteration: _,
                usage,
            } => Some(DashboardEvent::CostUpdate {
                task_id: session_id.to_string(),
                cost_usd: 0.0,
                tokens: usage.total(),
            }),

            AgentEvent::StateChange { new_state, .. } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("state: {:?}", new_state),
            }),

            AgentEvent::WaitingApproval { name, .. } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("waiting_approval: {}", name),
            }),

            AgentEvent::SubAgentSpawn { task } => Some(DashboardEvent::AgentActivity {
                task_id: session_id.to_string(),
                state: format!("spawn_subagent: {}", truncate(&task, 100)),
            }),

            // Events not useful for dashboard
            AgentEvent::ToolInput { .. }
            | AgentEvent::MessageEnd { .. }
            | AgentEvent::Finished { .. }
            | AgentEvent::Cleanup { .. } => None,
        }
    }
}

/// Truncate a string to max_len chars, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .take(max_len.saturating_sub(3))
            .last()
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        format!("{}...", &s[..end])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate("this is a very long string that exceeds the limit", 20);
        assert!(result.len() <= 23); // 17 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_to_dashboard_event_thinking() {
        let event = AgentEvent::Thinking {
            text: "analyzing codebase".to_string(),
        };
        let result = DashboardBridge::to_dashboard_event(event, "sess-1");
        assert!(result.is_some());
        let dash = result.unwrap();
        match dash {
            DashboardEvent::AgentActivity { task_id, state } => {
                assert_eq!(task_id, "sess-1");
                assert!(state.contains("thinking"));
            }
            _ => panic!("Expected AgentActivity"),
        }
    }

    #[test]
    fn test_to_dashboard_event_tool_start() {
        let event = AgentEvent::ToolStart {
            id: "t1".to_string(),
            name: "Read".to_string(),
        };
        let result = DashboardBridge::to_dashboard_event(event, "sess-1");
        assert!(result.is_some());
        match result.unwrap() {
            DashboardEvent::AgentActivity { state, .. } => {
                assert!(state.contains("Read"));
            }
            _ => panic!("Expected AgentActivity"),
        }
    }

    #[test]
    fn test_to_dashboard_event_done() {
        let event = AgentEvent::Done {
            final_text: "done".to_string(),
            iterations: 5,
            tool_calls: 12,
            total_usage: crate::providers::TokenUsage {
                input_tokens: 800,
                output_tokens: 200,
                reasoning_tokens: 0,
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            },
        };
        let result = DashboardBridge::to_dashboard_event(event, "sess-1");
        assert!(result.is_some());
        match result.unwrap() {
            DashboardEvent::AgentActivity { state, .. } => {
                assert!(state.contains("done"));
                assert!(state.contains("5 iterations"));
            }
            _ => panic!("Expected AgentActivity"),
        }
    }

    #[test]
    fn test_to_dashboard_event_iteration_cost() {
        let event = AgentEvent::IterationEnd {
            iteration: 3,
            usage: crate::providers::TokenUsage {
                input_tokens: 400,
                output_tokens: 100,
                reasoning_tokens: 0,
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            },
        };
        let result = DashboardBridge::to_dashboard_event(event, "sess-1");
        assert!(result.is_some());
        match result.unwrap() {
            DashboardEvent::CostUpdate {
                task_id, tokens, ..
            } => {
                assert_eq!(task_id, "sess-1");
                assert_eq!(tokens, 500);
            }
            _ => panic!("Expected CostUpdate"),
        }
    }

    #[test]
    fn test_to_dashboard_event_filtered() {
        // ToolInput should be filtered out (not useful for dashboard)
        let event = AgentEvent::ToolInput {
            json: "{}".to_string(),
        };
        assert!(DashboardBridge::to_dashboard_event(event, "sess-1").is_none());

        // MessageEnd should be filtered out
        let event = AgentEvent::MessageEnd {
            usage: crate::providers::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                reasoning_tokens: 0,
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            },
            stop_reason: crate::providers::StopReason::EndTurn,
        };
        assert!(DashboardBridge::to_dashboard_event(event, "sess-1").is_none());
    }
}
