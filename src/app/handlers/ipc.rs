//! IPC Event Handling

use anyhow::Result;
use std::time::Instant;
use tracing::{debug, error};

use crate::app::{App, ToolExecutionState};
use crate::ipc::{IpcEvent, Message, MessageRole, ToolCall, ToolStatus};

/// Route a tool-call update to the message that owns the tool_call id.
///
/// Tool-call ids are globally unique. Historically this was routed to
/// `messages.last_mut()`, which raced in multi-agent sessions: if another
/// agent pushed a message between the tool_call's creation and its update,
/// the update would land on the wrong message (or get appended as a
/// duplicate). We now search all messages newest→oldest so each update
/// always reaches its owner, regardless of interleaving.
///
/// If no message claims the id (a protocol-level anomaly in a well-formed
/// stream — `IpcEvent::Message` should always arrive first with the
/// tool_call embedded), we fall back to the newest message so the update
/// is not silently dropped.
pub(super) fn route_tool_call_update(messages: &mut Vec<Message>, tool_call: ToolCall) {
    let tool_call_id = tool_call.id.clone();
    let mut tool_call = Some(tool_call);

    for msg in messages.iter_mut().rev() {
        if let Some(existing) = msg
            .tool_calls
            .iter_mut()
            .find(|tc| tc.id == tool_call_id)
        {
            // take() here is infallible: matched on this iteration, haven't
            // moved `tool_call` yet, and we break immediately after.
            *existing = tool_call.take().expect("tool_call consumed exactly once");
            return;
        }
    }

    if let Some(tc) = tool_call {
        if let Some(last_msg) = messages.last_mut() {
            last_msg.tool_calls.push(tc);
        }
    }
}

impl App {
    /// Handle an IPC event
    pub async fn handle_ipc_event(&mut self, event: IpcEvent) -> Result<()> {
        match event {
            IpcEvent::Message(msg) => {
                // Check for tool_use blocks in the message and handle them
                for tc in &msg.tool_calls {
                    if tc.status == ToolStatus::Pending || tc.status == ToolStatus::Running {
                        debug!(tool_id = %tc.id, tool_name = %tc.name, "Received tool call");
                        // Trigger local tool execution if in standalone mode
                        if self.tools.standalone_tools_enabled && tc.status == ToolStatus::Pending {
                            let tool_call = tc.clone();
                            self.handle_tool_use(tool_call)?;
                        }
                    }
                }
                self.session.messages.push(msg);
                // Reset scroll when new message arrives
                self.ui.scroll_offset = 0;
            }
            IpcEvent::ToolCall(tool_call) => {
                debug!(
                    tool_id = %tool_call.id,
                    tool_name = %tool_call.name,
                    status = ?tool_call.status,
                    "Received tool call update"
                );

                // If tool is starting execution, track it
                if tool_call.status == ToolStatus::Running {
                    let exec_state = ToolExecutionState {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        input: tool_call.input.clone(),
                        start_time: Instant::now(),
                        is_executing: true,
                        output: tool_call.output.clone(),
                        is_error: tool_call.status == ToolStatus::Error,
                        elapsed_ms: tool_call.elapsed.unwrap_or(0),
                    };
                    // Check if already tracking this tool
                    if !self
                        .tools
                        .executing_tools
                        .iter()
                        .any(|s| s.id == tool_call.id)
                    {
                        self.tools.executing_tools.push(exec_state);
                    }
                }

                // If tool completed, move to recent_tools
                if tool_call.status == ToolStatus::Completed
                    || tool_call.status == ToolStatus::Error
                {
                    // Find and move the tool to recent_tools
                    if let Some(pos) = self
                        .tools
                        .executing_tools
                        .iter()
                        .position(|s| s.id == tool_call.id)
                    {
                        let mut tool = self.tools.executing_tools.remove(pos);
                        tool.is_executing = false;
                        tool.output = tool_call.output.clone();
                        tool.is_error = tool_call.status == ToolStatus::Error;
                        tool.elapsed_ms = tool_call.elapsed.unwrap_or(0);
                        // Keep only last 20 recent tools
                        if self.tools.recent_tools.len() >= 20 {
                            self.tools.recent_tools.remove(0);
                        }
                        self.tools.recent_tools.push(tool);
                    }
                }

                route_tool_call_update(&mut self.session.messages, tool_call);
            }
            IpcEvent::Thinking(state) => {
                // Track thinking start time
                if state.is_thinking && !self.session.thinking.is_thinking {
                    self.session.thinking_start = Some(Instant::now());
                } else if !state.is_thinking {
                    self.session.thinking_start = None;
                }
                self.session.thinking = state;
            }
            IpcEvent::PermissionRequest(req) => {
                self.session.permission_request = Some(req);
            }
            IpcEvent::Error(msg) => {
                error!("IPC error: {}", msg);
                // Add error message
                let error_msg = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: MessageRole::System,
                    content: format!("Error: {}", msg),
                    timestamp: chrono::Utc::now(),
                    is_error: true,
                    tool_calls: Vec::new(),
                    is_streaming: false,
                    shell_cmd: None,
                    exit_code: None,
                };
                self.session.messages.push(error_msg);
                self.check_queue()?;
            }
            IpcEvent::SessionEnd(mut usage) => {
                // Calculate cost if not provided by agent
                if usage.total_cost.is_none() {
                    let model = self.model.as_deref().unwrap_or("claude-3-5-sonnet");
                    usage.total_cost = Some(crate::agent::cost::calculate_cost(&usage, model));
                }
                self.session.token_usage = usage;
                self.check_queue()?;
            }
        }
        Ok(())
    }
}
