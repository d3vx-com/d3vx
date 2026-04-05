use super::{HookContext, HookError, HookResult, PreCommitHook};
use std::process::Command;

pub struct FormatCheck;

impl PreCommitHook for FormatCheck {
    fn name(&self) -> &str {
        "FormatCheck"
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        let output = Command::new("cargo")
            .arg("fmt")
            .arg("--check")
            .current_dir(&ctx.worktree_path)
            .output()?;

        if output.status.success() {
            Ok(HookResult::Pass)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(HookResult::Fail(format!(
                "cargo fmt failed:\n{}\n{}",
                stdout, stderr
            )))
        }
    }
}

pub struct ClippyCheck;

impl PreCommitHook for ClippyCheck {
    fn name(&self) -> &str {
        "ClippyCheck"
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        let output = Command::new("cargo")
            .arg("clippy")
            .arg("--all-targets")
            .arg("--")
            .arg("-D")
            .arg("warnings")
            .current_dir(&ctx.worktree_path)
            .output()?;

        if output.status.success() {
            Ok(HookResult::Pass)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(HookResult::Fail(format!(
                "cargo clippy failed:\n{}",
                stderr
            )))
        }
    }
}

pub struct TestCheck;

impl PreCommitHook for TestCheck {
    fn name(&self) -> &str {
        "TestCheck"
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        // Run all tests for now; optimizing to "affected tests" requires deep crate-graph knowledge
        let output = Command::new("cargo")
            .arg("test")
            .current_dir(&ctx.worktree_path)
            .output()?;

        if output.status.success() {
            Ok(HookResult::Pass)
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(HookResult::Fail(format!("cargo test failed:\n{}", stdout)))
        }
    }
}

pub struct SecurityCheck;

impl PreCommitHook for SecurityCheck {
    fn name(&self) -> &str {
        "SecurityCheck"
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        // Basic static secret detection across changed files
        let dangerous_patterns = vec!["AKIA", "ghp_", "sk_test_", "sk_live_"];

        for file in &ctx.changed_files {
            if !file.is_file() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(file) {
                for pattern in &dangerous_patterns {
                    if content.contains(pattern) {
                        return Ok(HookResult::Fail(format!(
                            "Possible secret found in {:?}: matches pattern {}",
                            file, pattern
                        )));
                    }
                }
            }
        }

        Ok(HookResult::Pass)
    }
}
