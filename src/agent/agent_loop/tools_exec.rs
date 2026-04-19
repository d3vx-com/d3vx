//! Tool execution: approval flow, concurrent execution, and diagnostic injection.

use std::collections::HashSet;

use tracing::debug;

use crate::agent::tool_coordinator::ToolExecutionResult;
use crate::config::types::SandboxMode;
use crate::ipc::types::{ApprovalDecision, ToolCall, ToolStatus};
use crate::store::{NewToolExecution, ToolExecutionStore};
use crate::tools::{ToolContext, ToolResult};
use crate::utils::diff::generate_unified_diff;

use super::types::AgentEvent;
use super::AgentLoop;

impl AgentLoop {
    /// Execute a batch of tool calls.
    pub(super) async fn execute_tools(
        &self,
        calls: Vec<(String, String, serde_json::Value)>,
        working_dir: &str,
        session_id: &str,
    ) -> Vec<ToolExecutionResult> {
        let (parent_session_id, delegation_depth, allow_parallel_spawn) = {
            let config = self.config.read().await;
            (
                config.parent_session_id.clone(),
                config.delegation_depth,
                config.allow_parallel_spawn,
            )
        };
        let mut context = ToolContext {
            cwd: working_dir.to_string(),
            env: std::env::vars().collect(),
            trust_mode: false,
            session_id: Some(session_id.to_string()),
            parent_session_id,
            agent_depth: delegation_depth,
            allow_parallel_spawn,
            bash_blocklist: vec![],
            sandbox_mode: SandboxMode::Disabled,
            sandbox_config: None,
            swarm_membership: None,
        };

        let mut approved_calls = Vec::new();
        let mut results = Vec::with_capacity(calls.len());
        let mut denied_indices = HashSet::new();

        // 1. Process approvals
        if let Some(ref guard) = self.guard {
            for (i, (id, name, input)) in calls.iter().enumerate() {
                let tool_call = ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    status: ToolStatus::Pending,
                    output: None,
                    elapsed: None,
                };

                let (diff, resource) = match self.generate_tool_diff(name, input, working_dir) {
                    Some((d, r)) => (Some(d), Some(r)),
                    None => (None, None),
                };

                let decision = match guard.prepare_approval(tool_call, diff, resource).await {
                    Ok(rx) => {
                        self.emit(AgentEvent::WaitingApproval {
                            id: id.clone(),
                            name: name.clone(),
                        });
                        guard.wait_for_decision(rx).await
                    }
                    Err(immediate_decision) => immediate_decision,
                };

                match decision {
                    ApprovalDecision::Approve => {
                        approved_calls.push((id.clone(), name.clone(), input.clone()));
                    }
                    ApprovalDecision::ApproveAll => {
                        approved_calls.push((id.clone(), name.clone(), input.clone()));
                        context.trust_mode = true;
                    }
                    ApprovalDecision::Deny => {
                        denied_indices.insert(i);
                        results.push(ToolExecutionResult {
                            id: id.clone(),
                            name: name.clone(),
                            result: ToolResult::error("Tool execution denied by user"),
                            elapsed_ms: 0,
                        });
                    }
                }
            }
        } else {
            approved_calls = calls.clone();
        }

        // Snapshot files before execution for undo support
        {
            let current_msg_count = self.conversation.read().await.len();
            let mut log = self.file_change_log.lock().await;
            log.snapshot_for(current_msg_count, &calls, working_dir);
        }

        // 2. Execute approved tools
        if !approved_calls.is_empty() {
            let exec_results = self
                .tools
                .execute_tools_concurrent(approved_calls, Some(&context))
                .await;

            // Merge results in order
            let mut exec_iter = exec_results.into_iter();
            let mut final_results = Vec::with_capacity(calls.len());

            for i in 0..calls.len() {
                if denied_indices.contains(&i) {
                    if let Some(denied) = results.iter().find(|r| r.id == calls[i].0) {
                        final_results.push(denied.clone());
                    }
                } else if let Some(res) = exec_iter.next() {
                    final_results.push(res);
                }
            }

            // Inject diagnostics after file-mutating tool calls
            self.inject_diagnostics(&mut final_results, working_dir)
                .await;

            // Audit: append each execution to the tool_executions table so
            // the dashboard and multi-agent attribution features see real
            // data. Best-effort — a missing session row or any DB error
            // must not fail tool execution, so we swallow errors after
            // logging. The table ships in SCHEMA_V1 but previously had no
            // writer; this closes that observability gap.
            self.record_tool_audit(&calls, &final_results, session_id)
                .await;

            final_results
        } else {
            results
        }
    }

    /// Append each completed tool call to the audit store. No-op if no
    /// database is configured.
    async fn record_tool_audit(
        &self,
        calls: &[(String, String, serde_json::Value)],
        results: &[ToolExecutionResult],
        session_id: &str,
    ) {
        let db_handle = {
            let config = self.config.read().await;
            match config.db.clone() {
                Some(h) => h,
                None => return,
            }
        };

        let db = db_handle.lock();
        let store = ToolExecutionStore::new(&db);

        for res in results {
            let input = calls
                .iter()
                .find(|(id, _, _)| id == &res.id)
                .map(|(_, _, input)| input.clone())
                .unwrap_or(serde_json::Value::Null);

            let record = NewToolExecution {
                session_id: session_id.to_string(),
                tool_name: res.name.clone(),
                tool_input: input,
                tool_result: Some(res.result.content.clone()),
                is_error: res.result.is_error,
                duration_ms: Some(res.elapsed_ms),
            };

            if let Err(e) = store.record(record) {
                // Expected for ephemeral sessions with no `sessions` row
                // (FK constraint). Log at debug so it's visible when
                // chasing observability gaps, silent in normal ops.
                debug!(
                    tool = %res.name,
                    session = %session_id,
                    error = %e,
                    "tool audit record skipped"
                );
            }
        }
    }

    /// Generate a unified diff for file-modifying tools. Returns (diff, file_path).
    pub(super) fn generate_tool_diff(
        &self,
        name: &str,
        input: &serde_json::Value,
        working_dir: &str,
    ) -> Option<(String, String)> {
        if name != "Write" && name != "Edit" {
            return None;
        }

        let file_path = input["file_path"].as_str()?;
        let path = if std::path::Path::new(file_path).is_absolute() {
            std::path::PathBuf::from(file_path)
        } else {
            std::path::Path::new(working_dir).join(file_path)
        };

        let old_content = std::fs::read_to_string(&path).ok()?;

        let new_content = if name == "Write" {
            input["content"].as_str().map(|s| s.to_string())
        } else if name == "Edit" {
            let old_string = input["old_string"].as_str()?;
            let new_string = input["new_string"].as_str()?;
            let replace_all = input["replace_all"].as_bool().unwrap_or(false);

            if replace_all {
                Some(old_content.replace(old_string, new_string))
            } else {
                old_content.find(old_string).map(|start| {
                    let end = start + old_string.len();
                    let mut new = String::with_capacity(
                        old_content.len() - old_string.len() + new_string.len(),
                    );
                    new.push_str(&old_content[..start]);
                    new.push_str(new_string);
                    new.push_str(&old_content[end..]);
                    new
                })
            }
        } else {
            None
        }?;

        Some((
            generate_unified_diff(file_path, &old_content, &new_content),
            file_path.to_string(),
        ))
    }
}
