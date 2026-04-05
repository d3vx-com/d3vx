//! IPC Event Handling

use anyhow::Result;
use std::time::Instant;
use tracing::{debug, error};

use crate::app::{App, ToolExecutionState};
use crate::ipc::{IpcEvent, Message, MessageRole, ToolStatus};

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

                // Find the latest message and add/update tool call
                if let Some(last_msg) = self.session.messages.last_mut() {
                    // Find existing tool call or add new one
                    if let Some(tc) = last_msg
                        .tool_calls
                        .iter_mut()
                        .find(|tc| tc.id == tool_call.id)
                    {
                        *tc = tool_call;
                    } else {
                        last_msg.tool_calls.push(tool_call);
                    }
                }
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
