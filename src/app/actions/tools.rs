//! Tool Execution Actions

use anyhow::Result;
use std::time::Instant;
use tracing::debug;

use crate::app::{App, ToolExecutionState};
use crate::config::types::SandboxMode;
use crate::ipc::{ToolCall, ToolStatus};
use crate::tools::ToolContext;

impl App {
    /// Handle a tool use from the agent
    pub fn handle_tool_use(&mut self, tool_call: ToolCall) -> Result<()> {
        // Update the tool call status to running
        self.update_tool_call_status(&tool_call.id, ToolStatus::Running);

        // Track execution state
        let exec_state = ToolExecutionState {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            input: tool_call.input.clone(),
            start_time: Instant::now(),
            is_executing: true,
            output: None,
            is_error: false,
            elapsed_ms: 0,
        };
        self.tools.executing_tools.push(exec_state);

        // If standalone mode, execute the tool locally
        if self.tools.standalone_tools_enabled {
            self.execute_tool_locally(tool_call)?;
        }

        Ok(())
    }

    /// Execute a tool locally using the ToolCoordinator
    pub fn execute_tool_locally(&mut self, tool_call: ToolCall) -> Result<()> {
        let coordinator = self.tools.tool_coordinator.clone();
        let tool_id = tool_call.id.clone();
        let tool_name = tool_call.name.clone();
        let tool_input = tool_call.input.clone();

        // Build tool context
        let context = ToolContext {
            cwd: self.cwd.clone().unwrap_or_else(|| ".".to_string()),
            env: std::env::vars().collect(),
            trust_mode: false,
            session_id: self.session.session_id.clone(),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            bash_blocklist: vec![],
            sandbox_mode: SandboxMode::Disabled,
            sandbox_config: None,
            swarm_membership: None,
        };

        // Spawn async task for tool execution
        let tool_id_clone = tool_id.clone();
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            debug!(tool_id = %tool_id_clone, tool_name = %tool_name, "Starting tool execution");

            let result = coordinator
                .execute_tool_with_timing(
                    tool_id_clone.clone(),
                    tool_name.clone(),
                    tool_input,
                    Some(&context),
                )
                .await;

            debug!(
                tool_id = %tool_id_clone,
                tool_name = %tool_name,
                elapsed_ms = result.elapsed_ms,
                is_error = result.result.is_error,
                "Tool execution completed"
            );

            if let Some(tx) = event_tx {
                let _ = tx
                    .send(crate::event::Event::ToolCompleted {
                        id: tool_id_clone,
                        output: result.result.content,
                        is_error: result.result.is_error,
                        elapsed_ms: result.elapsed_ms,
                    })
                    .await;
            }
        });

        Ok(())
    }

    /// Update a tool call's status in the message list
    pub fn update_tool_call_status(&mut self, tool_id: &str, status: ToolStatus) {
        for msg in &mut self.session.messages {
            for tc in &mut msg.tool_calls {
                if tc.id == tool_id {
                    tc.status = status;
                    return;
                }
            }
        }
    }

    /// Complete a tool call with a result
    pub fn complete_tool_call(
        &mut self,
        tool_id: &str,
        output: String,
        is_error: bool,
        elapsed_ms: u64,
    ) {
        // Update tool call in messages
        let mut found = false;
        'outer: for msg in &mut self.session.messages {
            for tc in &mut msg.tool_calls {
                if tc.id == tool_id {
                    tc.status = if is_error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Completed
                    };
                    tc.output = Some(output.clone());
                    tc.elapsed = Some(elapsed_ms);
                    found = true;
                    break 'outer;
                }
            }
        }

        if !found {
            // Check background workspace states
            'ws_outer: for state in self.workspace_states.values_mut() {
                for msg in &mut state.messages {
                    for tc in &mut msg.tool_calls {
                        if tc.id == tool_id {
                            tc.status = if is_error {
                                ToolStatus::Error
                            } else {
                                ToolStatus::Completed
                            };
                            tc.output = Some(output.clone());
                            tc.elapsed = Some(elapsed_ms);
                            break 'ws_outer;
                        }
                    }
                }
            }
        }

        // Move from executing to recent tools
        if let Some(pos) = self
            .tools
            .executing_tools
            .iter()
            .position(|s| s.id == tool_id)
        {
            let mut tool = self.tools.executing_tools.remove(pos);
            tool.is_executing = false;
            tool.output = Some(output);
            tool.is_error = is_error;
            tool.elapsed_ms = elapsed_ms;
            // Keep only last 20 recent tools
            if self.tools.recent_tools.len() >= 20 {
                self.tools.recent_tools.remove(0);
            }
            self.tools.recent_tools.push(tool);
        }
    }
}
