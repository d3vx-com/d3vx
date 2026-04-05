//! Hooks Execution Engine
//!
//! Evaluates and executes hooks for tool events.

use std::process::Stdio;
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::Mutex;

use super::auto_review::{self, AutoReviewConfig, QualityGateResult};
use super::prompt::PromptHookEvaluator;
use super::types::*;

/// Engine that manages and executes hooks.
pub struct HookEngine {
    hooks: Arc<Mutex<Vec<HookDefinition>>>,
}

impl HookEngine {
    /// Create a new empty hook engine.
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a new hook.
    pub async fn register(&self, hook: HookDefinition) {
        let mut hooks = self.hooks.lock().await;
        hooks.push(hook);
    }

    /// Remove a hook by name. Returns true if a hook was removed.
    pub async fn remove(&self, name: &str) -> bool {
        let mut hooks = self.hooks.lock().await;
        let before = hooks.len();
        hooks.retain(|h| h.name != name);
        hooks.len() != before
    }

    /// Get hooks registered for a specific event type.
    pub async fn hooks_for_event(&self, event: &HookEvent) -> Vec<HookDefinition> {
        let hooks = self.hooks.lock().await;
        hooks.iter().filter(|h| h.matches(event)).cloned().collect()
    }

    /// Run all hooks matching an event. Returns combined decision.
    ///
    /// If any hook returns Block, the overall result is Block.
    /// Hooks are executed in registration order.
    pub async fn run_hooks(&self, ctx: &HookExecutionContext) -> HookOutput {
        let matching = self.hooks_for_event(&ctx.event).await;
        if matching.is_empty() {
            return HookOutput::default();
        }

        let mut combined = HookOutput::default();

        for hook in matching {
            tracing::debug!(name = %hook.name, "Executing hook");
            let result = match &hook.kind {
                HookKind::Command { command } => self.execute_command_hook(command, ctx).await,
                HookKind::Prompt { template } => {
                    // Prompt-based hooks with template evaluation.
                    tracing::debug!(name = %hook.name, "Executing prompt hook");
                    let evaluator = PromptHookEvaluator::new();
                    evaluator.evaluate(template, ctx)
                }
            };
            combined.merge(result);
            // Short-circuit on block.
            if combined.decision == HookDecision::Block {
                break;
            }
        }

        combined
    }

    /// Execute a single command hook.
    ///
    /// Sets environment variables:
    /// - `D3VX_TOOL_NAME`: the tool name (if applicable)
    /// - `D3VX_SESSION_ID`: the session ID (if set)
    /// - `D3VX_WORKING_DIR`: the working directory
    ///
    /// Pipes tool_input as JSON to stdin.
    /// Parses stdout as JSON for decision/message/additionalContext.
    /// On failure, returns Pass to avoid blocking on hook errors.
    async fn execute_command_hook(&self, command: &str, ctx: &HookExecutionContext) -> HookOutput {
        let _tool_input_str = ctx
            .tool_input
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();

        let result = Command::new("sh")
            .arg("-c")
            .arg(command)
            .env("D3VX_WORKING_DIR", &ctx.working_dir)
            .env("D3VX_TOOL_NAME", ctx.event.tool_name().unwrap_or(""))
            .env("D3VX_SESSION_ID", ctx.session_id.as_deref().unwrap_or(""))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let output = match result.await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(error = %e, command = %command, "Hook command failed to start");
                return HookOutput::default();
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(
                command = %command,
                exit_code = output.status.code().unwrap_or(-1),
                stderr = %stderr,
                "Hook command exited with non-zero status"
            );
            return HookOutput::default();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_hook_output(&stdout)
    }

    /// Load hooks from config, replacing any existing hooks.
    pub async fn load_from_config(&self, hooks: Vec<HookDefinition>) {
        let mut guard = self.hooks.lock().await;
        *guard = hooks;
    }

    /// Clear all hooks.
    pub async fn clear(&self) {
        let mut guard = self.hooks.lock().await;
        guard.clear();
    }

    /// Run the automatic post-edit quality review for the given tool and files.
    ///
    /// Returns a formatted summary string if the tool triggers a review and
    /// there are findings, or `None` otherwise. This is designed to be called
    /// after an edit/write tool completes so that findings can be appended to
    /// the tool result or logged.
    pub fn run_post_edit_review(&self, tool_name: &str, file_paths: &[String]) -> Option<String> {
        if !auto_review::should_trigger_review(tool_name) {
            return None;
        }

        let config = AutoReviewConfig::default();
        let findings = auto_review::review_file_changes_with_config(file_paths, &config);

        if findings.is_empty() {
            return None;
        }

        let summary = auto_review::format_findings(&findings);
        tracing::info!(tool = %tool_name, findings = findings.len(), "Auto-review completed");
        Some(summary)
    }

    /// Run the post-edit quality gate after a tool call.
    ///
    /// This is the primary entry point for the post-tool-call quality gate.
    /// It delegates to [`auto_review::check_post_edit_quality`] which performs
    /// static checks and optional LSP diagnostics. If compilation errors are
    /// detected the returned `QualityGateResult` will have `has_errors` set.
    ///
    /// Returns `None` if the tool does not trigger a review or if no findings
    /// were produced.
    pub fn run_post_tool_call_quality_gate(
        &self,
        tool_name: &str,
        file_paths: &[String],
        config: &AutoReviewConfig,
    ) -> Option<QualityGateResult> {
        auto_review::check_post_edit_quality(tool_name, file_paths, config)
    }
}

impl Default for HookEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse stdout from a hook command into a HookOutput.
///
/// Expected JSON format:
/// ```json
/// {"decision": "approve"|"block", "message": "...", "additionalContext": "..."}
/// ```
///
/// If parsing fails, returns Pass (don't block on malformed output).
fn parse_hook_output(stdout: &str) -> HookOutput {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return HookOutput::default();
    }

    let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(
                stdout = %trimmed,
                error = %e,
                "Hook stdout is not valid JSON, treating as Pass"
            );
            return HookOutput::default();
        }
    };

    let decision = match parsed.get("decision").and_then(|v| v.as_str()) {
        Some("block") => HookDecision::Block,
        Some("approve") => HookDecision::Approve,
        _ => HookDecision::Pass,
    };

    let message = parsed
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let additional_context = parsed
        .get("additionalContext")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    HookOutput {
        decision,
        message,
        additional_context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_command_hook(name: &str, event: HookEvent, command: &str) -> HookDefinition {
        HookDefinition {
            name: name.to_string(),
            event,
            kind: HookKind::Command {
                command: command.to_string(),
            },
            enabled: true,
        }
    }

    fn make_ctx(event: HookEvent) -> HookExecutionContext {
        HookExecutionContext {
            event,
            tool_input: None,
            tool_output: None,
            working_dir: std::path::PathBuf::from("/tmp"),
            session_id: Some("test-session".to_string()),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn register_and_retrieve_hooks() {
        let engine = HookEngine::new();
        let hook = make_command_hook(
            "test-hook",
            HookEvent::PreToolUse {
                tool_name: "Bash".to_string(),
            },
            "echo ok",
        );
        engine.register(hook).await;

        let hooks = engine
            .hooks_for_event(&HookEvent::PreToolUse {
                tool_name: "Bash".to_string(),
            })
            .await;
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].name, "test-hook");
    }

    #[tokio::test]
    async fn remove_hook() {
        let engine = HookEngine::new();
        engine
            .register(make_command_hook(
                "to-remove",
                HookEvent::SessionStart,
                "echo hi",
            ))
            .await;

        assert!(engine.remove("to-remove").await);
        assert!(!engine.remove("nonexistent").await);

        let hooks = engine.hooks_for_event(&HookEvent::SessionStart).await;
        assert!(hooks.is_empty());
    }

    #[tokio::test]
    async fn block_decision_takes_priority() {
        let engine = HookEngine::new();

        // Register an approve hook.
        engine
            .register(make_command_hook(
                "approve-hook",
                HookEvent::PreToolUse {
                    tool_name: "Bash".to_string(),
                },
                r#"echo '{"decision":"approve"}'"#,
            ))
            .await;

        // Register a block hook.
        engine
            .register(make_command_hook(
                "block-hook",
                HookEvent::PreToolUse {
                    tool_name: "Bash".to_string(),
                },
                r#"echo '{"decision":"block","message":"denied"}'"#,
            ))
            .await;

        let ctx = make_ctx(HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        });
        let result = engine.run_hooks(&ctx).await;
        assert_eq!(result.decision, HookDecision::Block);
        assert_eq!(result.message.as_deref(), Some("denied"));
    }

    #[tokio::test]
    async fn approve_and_pass_combines_to_approve() {
        let engine = HookEngine::new();

        // Register an approve hook.
        engine
            .register(make_command_hook(
                "approve-hook",
                HookEvent::PreToolUse {
                    tool_name: "Bash".to_string(),
                },
                r#"echo '{"decision":"approve"}'"#,
            ))
            .await;

        let ctx = make_ctx(HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        });
        let result = engine.run_hooks(&ctx).await;
        assert_eq!(result.decision, HookDecision::Approve);
    }

    #[tokio::test]
    async fn command_hook_sets_env_vars() {
        let engine = HookEngine::new();

        // Hook that prints env vars to verify they are set.
        engine
            .register(make_command_hook(
                "env-check",
                HookEvent::PreToolUse {
                    tool_name: "Bash".to_string(),
                },
                r#"echo '{"decision":"approve","message":"'$D3VX_TOOL_NAME'-'$D3VX_SESSION_ID'"}'"#,
            ))
            .await;

        let ctx = make_ctx(HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        });
        let result = engine.run_hooks(&ctx).await;
        assert_eq!(result.decision, HookDecision::Approve);
        // The message should contain the env var values.
        let msg = result.message.unwrap_or_default();
        assert!(
            msg.contains("Bash"),
            "Expected tool name in message, got: {}",
            msg
        );
        assert!(
            msg.contains("test-session"),
            "Expected session id in message, got: {}",
            msg
        );
    }

    #[tokio::test]
    async fn clear_removes_all_hooks() {
        let engine = HookEngine::new();
        engine
            .register(make_command_hook("h1", HookEvent::SessionStart, "echo"))
            .await;
        engine
            .register(make_command_hook("h2", HookEvent::SessionStart, "echo"))
            .await;

        engine.clear().await;

        let hooks = engine.hooks_for_event(&HookEvent::SessionStart).await;
        assert!(hooks.is_empty());
    }

    #[tokio::test]
    async fn failed_command_returns_pass() {
        let engine = HookEngine::new();

        engine
            .register(make_command_hook(
                "fail-hook",
                HookEvent::SessionStart,
                "exit 1",
            ))
            .await;

        let ctx = make_ctx(HookEvent::SessionStart);
        let result = engine.run_hooks(&ctx).await;
        assert_eq!(result.decision, HookDecision::Pass);
    }

    #[tokio::test]
    async fn load_from_config_replaces_hooks() {
        let engine = HookEngine::new();
        engine
            .register(make_command_hook("old", HookEvent::SessionStart, "echo"))
            .await;

        let new_hooks = vec![make_command_hook(
            "new",
            HookEvent::Stop {
                reason: "done".to_string(),
            },
            "echo",
        )];
        engine.load_from_config(new_hooks).await;

        let session_hooks = engine.hooks_for_event(&HookEvent::SessionStart).await;
        assert!(session_hooks.is_empty());

        let stop_hooks = engine
            .hooks_for_event(&HookEvent::Stop {
                reason: "done".to_string(),
            })
            .await;
        assert_eq!(stop_hooks.len(), 1);
        assert_eq!(stop_hooks[0].name, "new");
    }

    #[test]
    fn parse_hook_output_valid_json() {
        let output = parse_hook_output(r#"{"decision":"approve"}"#);
        assert_eq!(output.decision, HookDecision::Approve);

        let output = parse_hook_output(r#"{"decision":"block","message":"nope"}"#);
        assert_eq!(output.decision, HookDecision::Block);
        assert_eq!(output.message.as_deref(), Some("nope"));
    }

    #[test]
    fn parse_hook_output_invalid_json_returns_pass() {
        let output = parse_hook_output("not json");
        assert_eq!(output.decision, HookDecision::Pass);
    }

    #[test]
    fn parse_hook_output_empty_returns_pass() {
        let output = parse_hook_output("");
        assert_eq!(output.decision, HookDecision::Pass);
    }

    #[test]
    fn parse_hook_output_with_additional_context() {
        let output =
            parse_hook_output(r#"{"decision":"approve","additionalContext":"extra info"}"#);
        assert_eq!(output.additional_context.as_deref(), Some("extra info"));
    }

    #[test]
    fn post_tool_call_quality_gate_returns_none_for_read() {
        let engine = HookEngine::new();
        let config = AutoReviewConfig::default();
        let result =
            engine.run_post_tool_call_quality_gate("read", &["foo.rs".to_string()], &config);
        assert!(
            result.is_none(),
            "Quality gate should not trigger for read tool"
        );
    }

    #[test]
    fn post_tool_call_quality_gate_detects_errors() {
        use std::io::Write as IoWrite;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("broken.rs");
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(b"fn main() {\n  let x = 1;\n").expect("write");

        let engine = HookEngine::new();
        let config = AutoReviewConfig {
            check_diagnostics: true,
            ..AutoReviewConfig::default()
        };
        let result = engine.run_post_tool_call_quality_gate(
            "edit",
            &[path.to_string_lossy().to_string()],
            &config,
        );
        assert!(
            result.is_some(),
            "Quality gate should trigger for edit tool"
        );
        assert!(
            result.unwrap().has_errors,
            "Expected errors from unmatched brace"
        );
    }
}
