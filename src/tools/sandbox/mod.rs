//! Sandbox Module
//!
//! OS-level sandboxing for tool command execution.
//! Provides platform-specific process isolation using:
//! - macOS: sandbox-exec (Seatbelt profiles)
//! - Linux: bubblewrap (bwrap)

use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tracing::{debug, warn};

use crate::config::types::{SandboxConfig, SandboxMode};

pub mod bubblewrap;
pub mod policy;
pub mod seatbelt;

// Re-export the trait so consumers can use it without knowing internals.
pub use bubblewrap::BubblewrapSandbox;
pub use seatbelt::SeatbeltSandbox;

/// Error type for sandbox operations.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// Failed to write the sandbox profile to a temporary file.
    #[error("profile generation failed: {0}")]
    ProfileWriteFailed(String),

    /// The required sandbox executable was not found on `$PATH`.
    #[error("sandbox executable not found: {0}")]
    ExecutableNotFound(String),

    /// The sandboxed command returned a non-zero exit code or could not start.
    #[error("sandbox command failed: {0}")]
    ExecutionFailed(String),

    /// Sandboxing is not available on the current platform.
    #[error("sandbox not available on this platform")]
    NotAvailable,
}

/// Captured output and metadata from a sandboxed command execution.
#[derive(Debug)]
pub struct SandboxResult {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Exit code (None if the process was killed by a signal).
    pub exit_code: Option<i32>,
    /// Wall-clock duration of the execution.
    pub duration: std::time::Duration,
}

/// Platform-agnostic trait for building sandboxed commands.
///
/// Implementors translate a [`SandboxConfig`] into a [`Command`] that wraps
/// the user-specified shell command inside the platform sandbox.
pub trait ProcessSandbox: Send + Sync {
    /// Build a [`Command`] that executes `cmd` inside the sandbox.
    ///
    /// The returned command, when spawned, will run `bash -c <cmd>` with
    /// restrictions derived from `config`.  The working directory is set to
    /// `cwd`.
    fn build_command(
        &self,
        cmd: &str,
        cwd: &Path,
        config: &SandboxConfig,
    ) -> Result<Command, SandboxError>;
}

/// Return the platform-appropriate sandbox executor.
///
/// # Panics (compile-time)
/// On platforms other than macOS or Linux the function body simply does not
/// compile, which is intentional -- there is no native sandbox to use.
pub fn platform_executor() -> Result<Box<dyn ProcessSandbox>, SandboxError> {
    #[cfg(target_os = "macos")]
    {
        debug!("selected seatbelt sandbox (macOS)");
        Ok(Box::new(SeatbeltSandbox))
    }
    #[cfg(target_os = "linux")]
    {
        debug!("selected bubblewrap sandbox (Linux)");
        Ok(Box::new(BubblewrapSandbox))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(SandboxError::NotAvailable)
    }
}

/// Execute a command according to the configured [`SandboxMode`].
///
/// - [`SandboxMode::Disabled`] -- runs the command directly, no wrapping.
/// - [`SandboxMode::Restricted`] -- runs directly (blocklist-only, no OS sandbox).
/// - [`SandboxMode::Native`] -- delegates to the platform sandbox executor.
pub fn execute_in_sandbox(
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
    config: &SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    match config.mode {
        SandboxMode::Disabled => execute_direct(command, cwd, env),
        SandboxMode::Restricted => {
            warn!("sandbox restricted mode: command sanitization only, no OS sandboxing");
            execute_direct(command, cwd, env)
        }
        SandboxMode::Native => execute_native(command, cwd, env, config),
    }
}

/// Run `bash -c <command>` without any sandbox wrapping.
fn execute_direct(
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
) -> Result<SandboxResult, SandboxError> {
    let start = Instant::now();
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .envs(env.iter().cloned())
        .output()
        .map_err(|e| SandboxError::ExecutionFailed(e.to_string()))?;

    Ok(SandboxResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
        duration: start.elapsed(),
    })
}

/// Run `bash -c <command>` inside the platform-native sandbox.
fn execute_native(
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
    config: &SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    let executor = platform_executor()?;
    let mut cmd = executor.build_command(command, cwd, config)?;
    cmd.envs(env.iter().cloned());

    let start = Instant::now();
    let output = cmd
        .output()
        .map_err(|e| SandboxError::ExecutionFailed(e.to_string()))?;

    Ok(SandboxResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
        duration: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{FilesystemRestriction, NetworkRestriction};
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn test_disabled_mode_runs_directly() {
        let config = SandboxConfig::default();
        assert_eq!(config.mode, SandboxMode::Disabled);

        let result = execute_in_sandbox("echo hello", &tmp_dir(), &[], &config);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.stdout.contains("hello"));
        assert_eq!(out.exit_code, Some(0));
    }

    #[test]
    fn test_restricted_mode_runs_directly() {
        let config = SandboxConfig {
            mode: SandboxMode::Restricted,
            enabled: true,
            ..Default::default()
        };

        let result = execute_in_sandbox("echo restricted", &tmp_dir(), &[], &config);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.stdout.contains("restricted"));
    }

    #[test]
    fn test_execute_direct_captures_stderr() {
        let config = SandboxConfig::default();
        let result = execute_in_sandbox("echo err >&2 && echo out", &tmp_dir(), &[], &config);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.stdout.contains("out"));
        assert!(out.stderr.contains("err"));
    }

    #[test]
    fn test_execute_direct_nonzero_exit() {
        let config = SandboxConfig::default();
        let result = execute_in_sandbox("exit 42", &tmp_dir(), &[], &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().exit_code, Some(42));
    }

    #[test]
    fn test_platform_executor_returns_ok() {
        // On macOS/Linux this must succeed; on other platforms it would error.
        let result = platform_executor();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert!(result.is_ok());
    }

    #[test]
    fn test_native_mode_builds_command() {
        let config = SandboxConfig {
            mode: SandboxMode::Native,
            enabled: true,
            network: NetworkRestriction::default(),
            filesystem: FilesystemRestriction::default(),
        };

        let executor = platform_executor();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let exec = executor.unwrap();
            let cmd = exec.build_command("echo native", &tmp_dir(), &config);
            assert!(cmd.is_ok());
        }
    }

    #[test]
    fn test_sandbox_error_display() {
        let err = SandboxError::ProfileWriteFailed("disk full".into());
        assert!(err.to_string().contains("disk full"));

        let err = SandboxError::ExecutableNotFound("bwrap".into());
        assert!(err.to_string().contains("bwrap"));

        let err = SandboxError::NotAvailable;
        assert!(err.to_string().contains("not available"));
    }
}
