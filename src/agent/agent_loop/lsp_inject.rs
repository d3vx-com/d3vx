//! LSP diagnostic injection after file-mutating tool calls.

use std::path::Path;

use super::AgentLoop;
use crate::agent::tool_coordinator::ToolExecutionResult;

/// The file-mutating tool names that should trigger diagnostics.
const MUTATING_TOOLS: &[&str] = &[
    "Edit",
    "EditFile",
    "Write",
    "WriteFile",
    "MultiEditTool",
    "RenameFile",
    "DeleteFile",
];

impl AgentLoop {
    /// Inject diagnostics into tool results after file mutations.
    ///
    /// Two-tier approach:
    /// 1. LSP bridge (fast, per-file, milliseconds) — injected first
    /// 2. Full compiler check (slower, comprehensive) — fallback if no bridge
    pub(super) async fn inject_diagnostics(
        &self,
        final_results: &mut [ToolExecutionResult],
        working_dir: &str,
    ) {
        if !final_results
            .iter()
            .any(|r| MUTATING_TOOLS.contains(&r.name.as_str()))
        {
            return;
        }

        // Tier 1: Fast LSP diagnostics (sub-second)
        if let Some(ref bridge) = self.lsp_bridge {
            self.inject_lsp_diagnostics(bridge, final_results).await;
        }

        // Tier 2: Fall through to compiler check (seconds)
        // This runs regardless and provides comprehensive validation
        self.inject_compiler_diagnostics(final_results, working_dir)
            .await;
    }

    /// Inject LSP diagnostics from the bridge (fast path).
    async fn inject_lsp_diagnostics(
        &self,
        bridge: &crate::lsp::LspBridge,
        results: &mut [ToolExecutionResult],
    ) {
        let last_mutating = results
            .iter()
            .rev()
            .find(|r| MUTATING_TOOLS.contains(&r.name.as_str()));

        if last_mutating.is_none() {
            return;
        }

        // Gather all touched files from mutating calls
        let mut all_diags = Vec::new();
        for result in results.iter() {
            if !MUTATING_TOOLS.contains(&result.name.as_str()) {
                continue;
            }

            // Extract file path from the result content and tool input
            let file_paths = extract_file_paths(&result.name, &result.result.content);
            for path_str in file_paths {
                let path = Path::new(&path_str);
                let abs_path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    Path::new(&result.name)
                        .parent()
                        .map(|p| p.join(path))
                        .unwrap_or_else(|| path.to_path_buf())
                };

                let diags = bridge.get_diagnostics(&abs_path).await;
                all_diags.extend(diags);
            }
        }

        if all_diags.is_empty() {
            return;
        }

        let formatted = crate::lsp::LspBridge::format_for_agent(&all_diags);
        if !formatted.is_empty() {
            if let Some(last_result) = results
                .iter_mut()
                .rev()
                .find(|r| MUTATING_TOOLS.contains(&r.name.as_str()))
            {
                last_result.result.content.push_str(&formatted);
            }
        }
    }

    /// Full compiler check diagnostics (existing slow path).
    async fn inject_compiler_diagnostics(
        &self,
        final_results: &mut [ToolExecutionResult],
        working_dir: &str,
    ) {
        let working_path = std::path::Path::new(working_dir);
        let is_rust = working_path.join("Cargo.toml").exists();
        let is_ts = working_path.join("tsconfig.json").exists();
        let is_go = working_path.join("go.mod").exists();

        let mut diagnostic_output = None;

        if is_rust {
            if let Ok(out) = tokio::process::Command::new("cargo")
                .arg("check")
                .arg("--color=never")
                .current_dir(working_dir)
                .output()
                .await
            {
                if !out.status.success() {
                    diagnostic_output = Some(String::from_utf8_lossy(&out.stderr).into_owned());
                }
            }
        } else if is_ts {
            if let Ok(out) = tokio::process::Command::new("npx")
                .args(["tsc", "--noEmit"])
                .current_dir(working_dir)
                .output()
                .await
            {
                if !out.status.success() {
                    diagnostic_output = Some(String::from_utf8_lossy(&out.stdout).into_owned());
                }
            }
        } else if is_go {
            if let Ok(out) = tokio::process::Command::new("go")
                .arg("build")
                .arg("./...")
                .current_dir(working_dir)
                .output()
                .await
            {
                if !out.status.success() {
                    diagnostic_output = Some(String::from_utf8_lossy(&out.stderr).into_owned());
                }
            }
        }

        if let Some(mut diag) = diagnostic_output {
            if let Some(last_result) = final_results
                .iter_mut()
                .rev()
                .find(|r| MUTATING_TOOLS.contains(&r.name.as_str()))
            {
                diag = format!("\n\n[Compiler Check]\n{}", diag);
                if diag.len() > 2000 {
                    diag.truncate(2000);
                    diag.push_str("\n... (truncated)");
                }
                last_result.result.content.push_str(&diag);
            }
        }
    }
}

/// Extract file paths from tool result content or name context.
fn extract_file_paths(_tool_name: &str, result_content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in result_content.lines() {
        // Skip the LSP/Compiler headers — we only want actual file references
        if line.contains("[LSP Diagnostics") || line.contains("[Compiler Check") {
            continue;
        }
        // Extract file path from standard formats like "src/main.rs:10:5"
        if let Some(pos) = line.find(|c| c == ':' || c == ' ') {
            let potential = &line[..pos];
            if potential.contains('/') || potential.contains('\\') {
                paths.push(potential.to_string());
            }
        }
    }
    paths
}
