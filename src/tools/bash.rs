//! Bash Tool
//!
//! Execute shell commands with timeout and output capture.

use async_trait::async_trait;
use std::process::Command;
use std::time::Instant;

use super::sandbox;
use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};
use crate::config::types::SandboxMode;

/// Bash tool for executing shell commands
pub struct BashTool {
    definition: ToolDefinition,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Bash".to_string(),
                description: concat!(
                    "Execute a bash shell command. ",
                    "Commands run in the current working directory. ",
                    "Use for system operations, git commands, npm/node, etc."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The bash command to execute"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Optional timeout in milliseconds (default: 120000)",
                            "default": 120000
                        },
                        "description": {
                            "type": "string",
                            "description": "Brief description of what the command does"
                        }
                    },
                    "required": ["command"]
                }),
            },
        }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let command = input["command"].as_str().unwrap_or("");
        let timeout_ms = input["timeout"].as_u64().unwrap_or(120_000);

        if command.is_empty() {
            return ToolResult::error("Command is required");
        }

        // Check blocklist before executing
        for pattern in &context.bash_blocklist {
            if pattern.is_match(command) {
                return ToolResult::error(format!(
                    "Command blocked by security policy: {}",
                    pattern
                ));
            }
        }

        // Intercept git commit commands to run Pre-commit hooks
        if command.starts_with("git commit") || command.contains(" git commit") {
            let config_options = crate::config::LoadConfigOptions {
                project_root: Some(context.cwd.clone()),
                ..Default::default()
            };
            if let Ok(config) = crate::config::load_config(config_options) {
                let hooks_cfg = config.git.pre_commit_hooks;

                let mut registry = crate::hooks::registry::HookRegistry::new();
                if hooks_cfg.format {
                    registry.register(Box::new(crate::hooks::checks::FormatCheck));
                }
                if hooks_cfg.clippy {
                    registry.register(Box::new(crate::hooks::checks::ClippyCheck));
                }
                if hooks_cfg.test {
                    registry.register(Box::new(crate::hooks::checks::TestCheck));
                }
                if hooks_cfg.security {
                    registry.register(Box::new(crate::hooks::checks::SecurityCheck));
                }

                // Get staged files
                let mut changed_files = Vec::new();
                if let Ok(diff_out) = std::process::Command::new("git")
                    .args(["diff", "--cached", "--name-only"])
                    .current_dir(&context.cwd)
                    .output()
                {
                    if diff_out.status.success() {
                        let files_str = String::from_utf8_lossy(&diff_out.stdout);
                        for line in files_str.lines() {
                            if !line.is_empty() {
                                let mut path = std::path::PathBuf::from(&context.cwd);
                                path.push(line);
                                changed_files.push(path);
                            }
                        }
                    }
                }

                let hook_ctx = crate::hooks::HookContext {
                    changed_files,
                    commit_message: "Pre-commit validation".to_string(),
                    worktree_path: context.cwd.clone().into(),
                };

                match registry.run_all(&hook_ctx) {
                    Ok(results) => {
                        let mut failures = Vec::new();
                        for (name, result) in results {
                            if let crate::hooks::HookResult::Fail(reason) = result {
                                failures.push(format!("{} failed:\n{}", name, reason));
                            }
                        }
                        if !failures.is_empty() {
                            return ToolResult::error(format!(
                                "COMMIT BLOCKED: Pre-commit hooks failed.\n\nPlease fix the following errors before committing:\n\n{}", 
                                failures.join("\n\n")
                            ));
                        }
                    }
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Failed to run internal pre-commit hooks: {}",
                            e
                        ));
                    }
                }
            }
        }

        // Execute the command — dispatch based on sandbox mode
        match context.sandbox_mode {
            SandboxMode::Disabled => {
                // Unchanged original behavior: direct execution
                let start = Instant::now();

                let output = Command::new("bash")
                    .arg("-c")
                    .arg(command)
                    .current_dir(&context.cwd)
                    .envs(&context.env)
                    .output();

                match output {
                    Ok(output) => {
                        let duration = start.elapsed();

                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);

                        let mut result = if output.status.success() {
                            let mut content = stdout.to_string();
                            if !stderr.is_empty() {
                                content.push_str("\n--- stderr ---\n");
                                content.push_str(&stderr);
                            }
                            ToolResult::success(content.trim())
                        } else {
                            let mut content = format!("Exit code: {:?}\n", output.status.code());
                            if !stdout.is_empty() {
                                content.push_str(&format!("--- stdout ---\n{}\n", stdout));
                            }
                            if !stderr.is_empty() {
                                content.push_str(&format!("--- stderr ---\n{}", stderr));
                            }
                            ToolResult::error(content.trim())
                        };

                        result = result
                            .with_metadata("duration_ms", serde_json::json!(duration.as_millis()))
                            .with_metadata("exit_code", serde_json::json!(output.status.code()));

                        result
                    }
                    Err(e) => {
                        if timeout_ms > 0 && start.elapsed().as_millis() as u64 > timeout_ms {
                            ToolResult::error(format!("Command timed out after {}ms", timeout_ms))
                        } else {
                            ToolResult::error(format!("Failed to execute command: {}", e))
                        }
                    }
                }
            }

            SandboxMode::Native => {
                // Wrap through sandbox module's platform-native sandboxing
                let config = match &context.sandbox_config {
                    Some(cfg) => cfg.clone(),
                    None => crate::config::types::SandboxConfig::default(),
                };
                let env_pairs: Vec<(String, String)> = context
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let cwd = std::path::PathBuf::from(&context.cwd);

                match sandbox::execute_in_sandbox(command, &cwd, &env_pairs, &config) {
                    Ok(sandbox_result) => {
                        let mut result = if sandbox_result.exit_code == Some(0) {
                            let mut content = sandbox_result.stdout.clone();
                            if !sandbox_result.stderr.is_empty() {
                                content.push_str("\n--- stderr ---\n");
                                content.push_str(&sandbox_result.stderr);
                            }
                            ToolResult::success(content.trim())
                        } else {
                            let mut content =
                                format!("Exit code: {:?}\n", sandbox_result.exit_code);
                            if !sandbox_result.stdout.is_empty() {
                                content.push_str(&format!(
                                    "--- stdout ---\n{}\n",
                                    sandbox_result.stdout
                                ));
                            }
                            if !sandbox_result.stderr.is_empty() {
                                content.push_str(&format!(
                                    "--- stderr ---\n{}",
                                    sandbox_result.stderr
                                ));
                            }
                            ToolResult::error(content.trim())
                        };

                        result = result
                            .with_metadata(
                                "duration_ms",
                                serde_json::json!(sandbox_result.duration.as_millis()),
                            )
                            .with_metadata("exit_code", serde_json::json!(sandbox_result.exit_code))
                            .with_metadata("sandbox", serde_json::json!("native"));

                        result
                    }
                    Err(e) => ToolResult::error(format!("Sandbox execution failed: {}", e)),
                }
            }

            SandboxMode::Restricted => {
                // Blocklist already applied above. Apply env sanitization only,
                // then run without OS-level sandboxing.
                let sanitized_env: std::collections::HashMap<String, String> = context
                    .env
                    .iter()
                    .filter(|(k, _)| {
                        // Strip potentially dangerous env vars
                        !k.starts_with("DOCKER")
                            && !k.starts_with("KUBERNETES")
                            && *k != "SSH_AUTH_SOCK"
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                let start = Instant::now();

                let output = Command::new("bash")
                    .arg("-c")
                    .arg(command)
                    .current_dir(&context.cwd)
                    .envs(&sanitized_env)
                    .output();

                match output {
                    Ok(output) => {
                        let duration = start.elapsed();

                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);

                        let mut result = if output.status.success() {
                            let mut content = stdout.to_string();
                            if !stderr.is_empty() {
                                content.push_str("\n--- stderr ---\n");
                                content.push_str(&stderr);
                            }
                            ToolResult::success(content.trim())
                        } else {
                            let mut content = format!("Exit code: {:?}\n", output.status.code());
                            if !stdout.is_empty() {
                                content.push_str(&format!("--- stdout ---\n{}\n", stdout));
                            }
                            if !stderr.is_empty() {
                                content.push_str(&format!("--- stderr ---\n{}", stderr));
                            }
                            ToolResult::error(content.trim())
                        };

                        result = result
                            .with_metadata("duration_ms", serde_json::json!(duration.as_millis()))
                            .with_metadata("exit_code", serde_json::json!(output.status.code()))
                            .with_metadata("sandbox", serde_json::json!("restricted"));

                        result
                    }
                    Err(e) => {
                        if timeout_ms > 0 && start.elapsed().as_millis() as u64 > timeout_ms {
                            ToolResult::error(format!("Command timed out after {}ms", timeout_ms))
                        } else {
                            ToolResult::error(format!("Failed to execute command: {}", e))
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = BashTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({"command": "echo 'Hello, World!'"}),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_bash_pwd() {
        let tool = BashTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"command": "pwd"}), &context)
            .await;

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_bash_blocklist_sudo() {
        let tool = BashTool::new();
        let context = ToolContext {
            bash_blocklist: vec![Regex::new(r"^sudo.*$").unwrap()],
            ..ToolContext::default()
        };

        let result = tool
            .execute(serde_json::json!({"command": "sudo rm -rf /"}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_bash_blocklist_rm_rf() {
        let tool = BashTool::new();
        let context = ToolContext {
            bash_blocklist: vec![Regex::new(r"^rm\s+-rf\s+.*$").unwrap()],
            ..ToolContext::default()
        };

        let result = tool
            .execute(
                serde_json::json!({"command": "rm -rf /home/user"}),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_bash_blocklist_multiple_patterns() {
        let tool = BashTool::new();
        let context = ToolContext {
            bash_blocklist: vec![
                Regex::new(r"^sudo.*$").unwrap(),
                Regex::new(r"^rm\s+-rf.*$").unwrap(),
                Regex::new(r"^chmod.*$").unwrap(),
            ],
            ..ToolContext::default()
        };

        // Test sudo is blocked
        let result = tool
            .execute(serde_json::json!({"command": "sudo ls"}), &context)
            .await;
        assert!(result.is_error);

        // Test chmod is blocked
        let result = tool
            .execute(serde_json::json!({"command": "chmod 777 file"}), &context)
            .await;
        assert!(result.is_error);

        // Test safe command passes
        let result = tool
            .execute(serde_json::json!({"command": "ls -la"}), &context)
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_bash_blocklist_empty_list() {
        let tool = BashTool::new();
        let context = ToolContext {
            bash_blocklist: vec![],
            ..ToolContext::default()
        };

        // Commands should work when blocklist is empty
        let result = tool
            .execute(serde_json::json!({"command": "echo test"}), &context)
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_bash_blocklist_partial_match_not_blocked() {
        let tool = BashTool::new();
        let context = ToolContext {
            bash_blocklist: vec![Regex::new(r"^sudo.*$").unwrap()],
            ..ToolContext::default()
        };

        // "sudo" in the middle should not be blocked (pattern is anchored to start)
        let result = tool
            .execute(serde_json::json!({"command": "echo sudo"}), &context)
            .await;
        assert!(!result.is_error);
    }
}
