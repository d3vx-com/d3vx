//! Workspace Post-Create Hooks
//!
//! Configurable setup actions that run after a git worktree is created.
//! Supports symlinks, shell commands, environment file setup, and file copies.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Configuration for workspace post-creation hooks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceHooksConfig {
    #[serde(default)]
    pub symlinks: Vec<SymlinkEntry>,
    #[serde(default)]
    pub commands: Vec<HookCommand>,
    #[serde(default)]
    pub copy_files: Vec<String>,
}

impl Default for WorkspaceHooksConfig {
    fn default() -> Self {
        Self {
            symlinks: Vec::new(),
            commands: Vec::new(),
            copy_files: Vec::new(),
        }
    }
}

/// A symlink to create in the worktree
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymlinkEntry {
    /// Source path relative to project root
    pub source: String,
    /// Target path relative to worktree root
    pub target: String,
    /// Whether to overwrite existing (default: false)
    #[serde(default)]
    pub overwrite: bool,
}

/// A shell command to run after worktree creation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookCommand {
    /// Shell command to execute
    pub command: String,
    /// Working directory relative to worktree root (default: ".")
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Timeout in seconds (default: 120)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Whether to continue on failure (default: false)
    #[serde(default)]
    pub continue_on_error: bool,
    /// Description for logging
    #[serde(default)]
    pub description: Option<String>,
}

/// Result of running workspace hooks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceHookResult {
    pub symlinks_created: usize,
    pub commands_run: usize,
    pub commands_failed: usize,
    pub files_copied: usize,
    pub errors: Vec<String>,
}

impl Default for WorkspaceHookResult {
    fn default() -> Self {
        Self {
            symlinks_created: 0,
            commands_run: 0,
            commands_failed: 0,
            files_copied: 0,
            errors: Vec::new(),
        }
    }
}

/// Executor for workspace post-create hooks
pub struct WorkspaceHookExecutor;

impl WorkspaceHookExecutor {
    /// Run all post-create hooks for a new worktree
    pub fn execute(
        project_root: &Path,
        worktree_path: &Path,
        config: &WorkspaceHooksConfig,
    ) -> WorkspaceHookResult {
        let mut result = WorkspaceHookResult::default();
        info!(
            "Running workspace post-create hooks for {}",
            worktree_path.display()
        );

        let (created, errs) = Self::create_symlinks(project_root, worktree_path, &config.symlinks);
        result.symlinks_created = created;
        result.errors.extend(errs);

        let (copied, errs) = Self::copy_files(project_root, worktree_path, &config.copy_files);
        result.files_copied = copied;
        result.errors.extend(errs);

        let (ran, failed, errs) = Self::run_commands(worktree_path, &config.commands);
        result.commands_run = ran;
        result.commands_failed = failed;
        result.errors.extend(errs);

        info!(
            "Workspace hooks complete: {} symlinks, {} files copied, {}/{} commands ok",
            result.symlinks_created,
            result.files_copied,
            result.commands_run.saturating_sub(result.commands_failed),
            result.commands_run,
        );
        result
    }

    fn create_symlinks(
        project_root: &Path,
        worktree_path: &Path,
        symlinks: &[SymlinkEntry],
    ) -> (usize, Vec<String>) {
        let mut created = 0usize;
        let mut errors = Vec::new();
        for entry in symlinks {
            let source = project_root.join(&entry.source);
            let target = worktree_path.join(&entry.target);
            if !source.exists() {
                errors.push(format!(
                    "Symlink source does not exist: {}",
                    source.display()
                ));
                continue;
            }
            if target.exists() || target.symlink_metadata().is_ok() {
                if entry.overwrite {
                    if let Err(e) = std::fs::remove_file(&target) {
                        errors.push(format!(
                            "Failed to remove existing symlink target {}: {}",
                            target.display(),
                            e
                        ));
                        continue;
                    }
                } else {
                    errors.push(format!(
                        "Symlink target already exists (and overwrite=false): {}",
                        target.display()
                    ));
                    continue;
                }
            }
            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    errors.push(format!(
                        "Failed to create parent dir for {}: {}",
                        target.display(),
                        e
                    ));
                    continue;
                }
            }
            match symlink(&source, &target) {
                Ok(()) => {
                    info!(
                        "Created symlink: {} -> {}",
                        target.display(),
                        source.display()
                    );
                    created += 1;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to create symlink {} -> {}: {}",
                        target.display(),
                        source.display(),
                        e
                    ));
                }
            }
        }
        (created, errors)
    }

    fn run_commands(worktree_path: &Path, commands: &[HookCommand]) -> (usize, usize, Vec<String>) {
        let mut run = 0usize;
        let mut failed = 0usize;
        let mut errors = Vec::new();
        for cmd in commands {
            let desc = cmd.description.as_deref().unwrap_or(&cmd.command);
            let working_dir = cmd
                .working_dir
                .as_ref()
                .map(|d| worktree_path.join(d))
                .unwrap_or_else(|| worktree_path.to_path_buf());
            let timeout = Duration::from_secs(cmd.timeout_secs.unwrap_or(120));

            info!("Running workspace hook command: {}", desc);
            let start = Instant::now();
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd.command)
                .current_dir(&working_dir)
                .output();
            let elapsed = start.elapsed();

            match output {
                Ok(out) if out.status.success() => {
                    info!(
                        "Command '{}' succeeded in {:.1}s",
                        desc,
                        elapsed.as_secs_f64()
                    );
                    run += 1;
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    errors.push(format!(
                        "Command '{}' failed (exit {}): {}",
                        desc,
                        out.status.code().unwrap_or(-1),
                        stderr.trim()
                    ));
                    run += 1;
                    failed += 1;
                    if !cmd.continue_on_error {
                        break;
                    }
                }
                Err(e) => {
                    errors.push(format!("Command '{}' failed to execute: {}", desc, e));
                    run += 1;
                    failed += 1;
                    if !cmd.continue_on_error {
                        break;
                    }
                }
            }
            if elapsed > timeout {
                warn!(
                    "Command '{}' exceeded timeout ({:.1}s > {}s)",
                    desc,
                    elapsed.as_secs_f64(),
                    timeout.as_secs()
                );
            }
        }
        (run, failed, errors)
    }

    fn copy_files(
        project_root: &Path,
        worktree_path: &Path,
        files: &[String],
    ) -> (usize, Vec<String>) {
        let mut copied = 0usize;
        let mut errors = Vec::new();
        for file_rel in files {
            let source = project_root.join(file_rel);
            let target = worktree_path.join(file_rel);
            if !source.exists() {
                errors.push(format!("Copy source does not exist: {}", source.display()));
                continue;
            }
            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    errors.push(format!(
                        "Failed to create parent dir for {}: {}",
                        target.display(),
                        e
                    ));
                    continue;
                }
            }
            match std::fs::copy(&source, &target) {
                Ok(_) => {
                    info!("Copied {} to worktree", file_rel);
                    copied += 1;
                }
                Err(e) => {
                    errors.push(format!("Failed to copy {}: {}", source.display(), e));
                }
            }
        }
        (copied, errors)
    }
}

/// Read workspace hooks config from `.d3vx/workspace.yml` or fall back to defaults
pub fn load_workspace_config(project_root: &Path) -> WorkspaceHooksConfig {
    let config_path = project_root.join(".d3vx").join("workspace.yml");
    if !config_path.exists() {
        debug!(
            "No workspace config at {}, using defaults",
            config_path.display()
        );
        return WorkspaceHooksConfig::default();
    }
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match serde_yaml::from_str(&contents) {
            Ok(config) => {
                info!(
                    "Loaded workspace hooks config from {}",
                    config_path.display()
                );
                config
            }
            Err(e) => {
                warn!(
                    "Failed to parse {}: {}, using defaults",
                    config_path.display(),
                    e
                );
                WorkspaceHooksConfig::default()
            }
        },
        Err(e) => {
            warn!(
                "Failed to read {}: {}, using defaults",
                config_path.display(),
                e
            );
            WorkspaceHooksConfig::default()
        }
    }
}

/// Resolve project root by walking up from cwd looking for `.git` or `.d3vx`
pub fn resolve_project_root() -> Option<PathBuf> {
    std::env::current_dir().ok().and_then(|cwd| {
        let mut current = cwd.as_path();
        loop {
            if current.join(".git").exists() || current.join(".d3vx").exists() {
                return Some(current.to_path_buf());
            }
            current = current.parent()?;
        }
    })
}
