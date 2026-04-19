//! Shell command execution and control actions (queue, approval, stop)

use anyhow::Result;
use std::time::Instant;
use tracing::info;

use crate::app::App;
use crate::event::Event;
use crate::ipc::{Message, MessageRole, ThinkingState};

/// Execute a bash command directly from input (!)
pub(super) fn execute_bash_command(app: &mut App, content: &str) -> Result<()> {
    let cmd = &content[1..].trim();
    if cmd.is_empty() {
        return Ok(());
    }

    // Add shell message
    let shell_msg = Message::shell(cmd.to_string(), format!("Running: {}...", cmd));
    app.session.messages.push(shell_msg.clone());

    let cwd = app.cwd.clone().unwrap_or_else(|| ".".to_string());
    let env = std::env::vars().collect::<std::collections::HashMap<String, String>>();
    let cmd_str = cmd.to_string();

    // Reset thinking for background work
    app.session.thinking = ThinkingState {
        is_thinking: true,
        text: format!("Running {}", cmd_str),
        phase: crate::ipc::types::ThinkingPhase::Thinking,
    };
    app.session.thinking_start = Some(Instant::now());

    let event_tx_clone = app.event_tx.clone();

    tokio::spawn(async move {
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd_str)
            .current_dir(cwd)
            .envs(env)
            .output();

        let (out_text, exit_code) = match output {
            Ok(out) => {
                let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
                if !out.stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str("--- stderr ---\n");
                    combined.push_str(&String::from_utf8_lossy(&out.stderr));
                }
                (combined, out.status.code().unwrap_or(0))
            }
            Err(e) => (format!("Failed to execute command: {}", e), 1),
        };

        if let Some(tx) = event_tx_clone {
            let _ = tx
                .send(Event::ShellResult {
                    cmd: cmd_str,
                    output: out_text,
                    exit_code,
                })
                .await;
        }
    });

    Ok(())
}

impl App {
    /// Handle shell command result
    pub fn handle_shell_result(
        &mut self,
        cmd: String,
        output: String,
        exit_code: i32,
    ) -> Result<()> {
        self.session.thinking = ThinkingState::default();
        self.session.thinking_start = None;

        let cmd_key = cmd.clone();
        if let Some(msg) = self
            .session
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.role == MessageRole::Shell && m.shell_cmd.as_deref() == Some(&cmd_key))
        {
            msg.content = output;
            msg.exit_code = Some(exit_code);
            msg.is_error = exit_code != 0;
            msg.is_streaming = false;
        } else {
            let mut msg = Message::shell(cmd, output);
            msg.exit_code = Some(exit_code);
            msg.is_error = exit_code != 0;
            self.session.messages.push(msg);
        }

        self.ui.scroll_offset = 0;
        self.check_queue()?;

        // Trigger save
        if self.agents.agent_loop.is_some() {
            if let Some(tx) = &self.event_tx {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(Event::SaveSession).await;
                });
            }
        }

        Ok(())
    }

    /// Check if there are queued messages and execute the next one
    pub fn check_queue(&mut self) -> Result<()> {
        if !self.session.message_queue.is_empty() && !self.session.thinking.is_thinking {
            let next_msg = self.session.message_queue.remove(0);
            self.execute_message(next_msg)?;
        }
        Ok(())
    }

    /// Respond to a permission request
    pub fn respond_permission(&mut self, response: &str) -> Result<()> {
        if let Some(req) = self.session.permission_request.take() {
            let request_id = req.id;
            if let Some(ref client) = self.ipc_client {
                let client = client.clone();
                let response = response.to_string();
                tokio::spawn(async move {
                    let _ = client.respond_permission(&request_id, &response).await;
                });
            }
        }
        Ok(())
    }

    /// Respond to a command approval request (Unified Approval)
    pub fn respond_approval(&mut self, decision: crate::ipc::ApprovalDecision) -> Result<()> {
        if let Some(req) = self.session.permission_request.take() {
            let tool_call_id = req.tool_call_id.clone().unwrap_or(req.id.clone());

            // 1. First try the main agent loop (for main chat mode)
            if let Some(ref agent) = self.agents.agent_loop {
                if let Some(guard) = &agent.guard {
                    let guard = guard.clone();
                    let tool_call_id_clone = tool_call_id.clone();
                    tokio::spawn(async move {
                        if !guard.provide_decision(&tool_call_id_clone, decision).await {
                            tracing::warn!(id = %tool_call_id_clone, "Failed to provide decision - request not found in main guard");
                        }
                    });
                    return Ok(());
                }
            }

            // 2. Fallback: Try to find in workspace agents
            if let Some(ws_id) = &req.workspace_id {
                if let Some(agent) = self.agents.workspace_agents.get(ws_id).cloned() {
                    if let Some(guard) = &agent.guard {
                        let guard = guard.clone();
                        tokio::spawn(async move {
                            if !guard.provide_decision(&tool_call_id, decision).await {
                                tracing::warn!(id = %tool_call_id, "Failed to provide decision - request not found in workspace guard");
                            }
                        });
                        return Ok(());
                    }
                }
            }

            // 3. Fallback for external IPC mode
            if let Some(ref client) = self.ipc_client {
                let response = match decision {
                    crate::ipc::ApprovalDecision::Approve => "approve",
                    crate::ipc::ApprovalDecision::Deny => "deny",
                    crate::ipc::ApprovalDecision::ApproveAll => "approve_all",
                };
                let client = client.clone();
                let request_id = req.id;
                tokio::spawn(async move {
                    let _ = client.respond_permission(&request_id, response).await;
                });
            }
        }
        Ok(())
    }

    /// Stop the current conversation/generation
    pub fn stop_conversation(&mut self) -> Result<()> {
        self.stop_agent(false)
    }

    pub fn stop_agent(&mut self, silent: bool) -> Result<()> {
        info!("Stopping agent (silent: {})", silent);

        self.session.thinking = ThinkingState::default();
        self.session.thinking_start = None;
        self.agents.streaming_message.clear();

        // Fire-and-forget the graceful stop. The previous implementation
        // called `Handle::current().block_on(…)` here, which panics with
        // "Cannot start a runtime from within a runtime" — because this
        // method is reached synchronously from the async `handle_key_event`
        // chain (e.g. Ctrl+C → stop_conversation → stop_agent). The panic
        // unwound past the terminal-cleanup step in the runner, leaving
        // the shell in raw mode + mouse capture until the user forcibly
        // reset it.
        //
        // `agent.stop()` only writes `paused = true` on an `Arc<RwLock>`
        // — there's nothing to wait on, so spawning the completion is
        // both safe and semantically correct. If no runtime is active
        // (defensive edge case; today the only callers are async), we
        // skip the async stop and let the agent observe the state change
        // on its next iteration.
        if let Some(agent) = &self.agents.agent_loop {
            let agent = agent.clone();
            if tokio::runtime::Handle::try_current().is_ok() {
                tokio::spawn(async move {
                    agent.stop().await;
                });
            } else {
                tracing::debug!(
                    "stop_agent called outside a tokio runtime; skipping async stop"
                );
            }
        }

        if !silent {
            self.add_system_message("Conversation stopped by user");
        }

        Ok(())
    }
}
