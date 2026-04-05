//! Commander Validation Runner
//!
//! Executes validation commands and collects structured results.

use std::path::PathBuf;
use std::time::Instant;

use tracing::{debug, info, warn};

use super::types::{ValidationCommand, ValidationKind, ValidationResult};

/// Executes validation commands and collects structured results.
pub struct ValidationRunner {
    pub(crate) commands: Vec<ValidationCommand>,
    pub(crate) project_root: PathBuf,
}

impl ValidationRunner {
    /// Create a runner with default validation commands.
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            commands: Self::default_commands(),
            project_root,
        }
    }

    /// Create a runner with custom validation commands.
    pub fn with_commands(project_root: PathBuf, commands: Vec<ValidationCommand>) -> Self {
        Self {
            commands,
            project_root,
        }
    }

    /// Return the standard set of validation commands.
    pub fn default_commands() -> Vec<ValidationCommand> {
        vec![
            ValidationCommand::type_check(),
            ValidationCommand::test(),
            ValidationCommand::lint(),
        ]
    }

    /// Run all validations concurrently using tokio tasks.
    pub async fn run_all(&self) -> Vec<ValidationResult> {
        info!(
            project = ?self.project_root,
            count = self.commands.len(),
            "Starting validation run"
        );

        let mut handles = Vec::with_capacity(self.commands.len());

        for cmd in &self.commands {
            let cmd = cmd.clone();
            let project_root = self.project_root.clone();
            let handle = tokio::spawn(async move {
                let runner = ValidationRunner {
                    commands: vec![],
                    project_root,
                };
                runner.run_one(&cmd).await
            });
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(err) => {
                    warn!(error = %err, "Validation task panicked");
                    results.push(ValidationResult {
                        kind: ValidationKind::Custom("unknown".to_string()),
                        success: false,
                        output: format!("Task panicked: {err}"),
                        duration_ms: 0,
                        errors: vec![format!("Task panicked: {err}")],
                        warnings: vec![],
                    });
                }
            }
        }

        let passed = results.iter().filter(|r| r.success).count();
        info!(total = results.len(), passed, "Validation run completed");

        results
    }

    /// Run a single validation command synchronously.
    pub async fn run_one(&self, cmd: &ValidationCommand) -> ValidationResult {
        info!(kind = %cmd.kind, command = %cmd.command, "Running validation");

        // Split command into program and args.
        let parts: Vec<&str> = cmd.command.split_whitespace().collect();
        if parts.is_empty() {
            return ValidationResult {
                kind: cmd.kind.clone(),
                success: false,
                output: "Empty command".to_string(),
                duration_ms: 0,
                errors: vec!["Empty command".to_string()],
                warnings: vec![],
            };
        }

        let program = parts[0];
        let args: Vec<&str> = parts[1..].to_vec();

        let start = Instant::now();

        let output_result = tokio::time::timeout(
            std::time::Duration::from_secs(cmd.timeout_secs),
            tokio::process::Command::new(program)
                .args(&args)
                .current_dir(&self.project_root)
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match output_result {
            Ok(Ok(output)) => {
                let success = output.status.success();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stdout.is_empty() {
                    stderr.clone()
                } else if stderr.is_empty() {
                    stdout.clone()
                } else {
                    format!("{stdout}\n{stderr}")
                };

                let (errors, warnings) = parse_output_issues(&combined, success);

                debug!(
                    kind = %cmd.kind,
                    success,
                    duration_ms,
                    "Validation completed"
                );

                ValidationResult {
                    kind: cmd.kind.clone(),
                    success,
                    output: combined,
                    duration_ms,
                    errors,
                    warnings,
                }
            }
            Ok(Err(err)) => {
                warn!(kind = %cmd.kind, error = %err, "Failed to execute command");
                ValidationResult {
                    kind: cmd.kind.clone(),
                    success: false,
                    output: format!("Execution failed: {err}"),
                    duration_ms,
                    errors: vec![format!("Execution failed: {err}")],
                    warnings: vec![],
                }
            }
            Err(_) => {
                warn!(kind = %cmd.kind, timeout = cmd.timeout_secs, "Validation timed out");
                ValidationResult {
                    kind: cmd.kind.clone(),
                    success: false,
                    output: format!("Timed out after {} seconds", cmd.timeout_secs),
                    duration_ms,
                    errors: vec![format!("Timed out after {} seconds", cmd.timeout_secs)],
                    warnings: vec![],
                }
            }
        }
    }

    /// Check if all results passed.
    pub fn all_passed(results: &[ValidationResult]) -> bool {
        results.iter().all(|r| r.success)
    }

    /// Produce a human-readable summary of results.
    pub fn summarize(results: &[ValidationResult]) -> String {
        if results.is_empty() {
            return "No validation results to summarize".to_string();
        }

        let total = results.len();
        let passed = results.iter().filter(|r| r.success).count();
        let failed = total - passed;

        let mut lines = Vec::with_capacity(total + 3);
        lines.push(format!("Validation Summary: {passed}/{total} passed"));

        for result in results {
            let status = if result.success { "PASS" } else { "FAIL" };
            let errors = if result.errors.is_empty() {
                String::new()
            } else {
                format!(" ({} errors)", result.errors.len())
            };
            lines.push(format!(
                "  [{status}] {} ({:.1}s){errors}",
                result.kind,
                result.duration_ms as f64 / 1000.0,
            ));
        }

        if failed > 0 {
            lines.push(format!("{failed} validation(s) failed"));
        }

        lines.join("\n")
    }
}

/// Parse compiler/linter output into separate error and warning lists.
pub(crate) fn parse_output_issues(output: &str, success: bool) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for line in output.lines() {
        let lower = line.to_lowercase();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Error patterns from cargo/rustc/clippy.
        if lower.contains("error[") || lower.contains("error:") {
            errors.push(trimmed.to_string());
        } else if lower.contains("warning:") || lower.contains("warn[") {
            warnings.push(trimmed.to_string());
        }
    }

    // If the command failed but we found no explicit error lines, treat the
    // whole output as one error.
    if !success && errors.is_empty() {
        // Take up to 3 lines of output as error context.
        let first_lines: Vec<String> = output.lines().take(3).map(String::from).collect();
        if first_lines.is_empty() {
            errors.push("Command failed with no output".to_string());
        } else {
            errors.push(first_lines.join("\n"));
        }
    }

    (errors, warnings)
}
