//! Multi-Runtime Support
//!
//! Concrete runtime implementations for agent process execution:
//! - `ProcessRuntime`: Direct child process management via `tokio::process::Command`.
//! - `TmuxRuntime`: Tmux session-based isolation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::plugin::{
    Plugin, PluginContext, PluginDescriptor, PluginError, PluginSlot,
    RuntimeAdapter as RuntimePlugin,
};

// ---------------------------------------------------------------------------
// ProcessRuntime -- direct child process management
// ---------------------------------------------------------------------------

/// Manages agent processes as direct child processes.
pub struct ProcessRuntime {
    processes: Arc<Mutex<HashMap<String, u32>>>,
    output_dir: PathBuf,
}

impl ProcessRuntime {
    /// Create a new `ProcessRuntime` that stores output in the system temp dir.
    pub fn new() -> Self {
        Self::with_output_dir(std::env::temp_dir())
    }

    /// Create a new `ProcessRuntime` with a specific output directory.
    pub fn with_output_dir(output_dir: PathBuf) -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
        }
    }

    /// Build the output file path for a task.
    fn output_path(&self, task_id: &str) -> PathBuf {
        self.output_dir.join(format!("d3vx-{}.out", task_id))
    }
}

impl Default for ProcessRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ProcessRuntime {
    fn name(&self) -> &str {
        "process-runtime"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn description(&self) -> Option<&str> {
        Some("Direct child process runtime")
    }
    fn init(&self, _context: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}

impl crate::plugin::AdapterPlugin for ProcessRuntime {
    fn slot(&self) -> PluginSlot {
        PluginSlot::Runtime
    }
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            name: self.name().into(),
            version: self.version().into(),
            slot: self.slot(),
            description: self.description().unwrap_or_default().into(),
        }
    }
}

#[async_trait]
impl RuntimePlugin for ProcessRuntime {
    async fn start(&self, task_id: &str, command: &str) -> Result<String, PluginError> {
        let output_file = self.output_path(task_id);

        // Build a shell command that redirects stdout+stderr to the output file.
        let shell_cmd = format!("{} > {} 2>&1 & echo $!", command, output_file.display());

        debug!("ProcessRuntime starting task {}: {}", task_id, command);

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .output()
            .await
            .map_err(|e| PluginError::Execution(format!("Failed to spawn process: {}", e)))?;

        let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let pid: u32 = pid_str.parse().map_err(|_| {
            PluginError::Execution(format!("Failed to parse PID from: {}", pid_str))
        })?;

        info!("ProcessRuntime started task {} with PID {}", task_id, pid);

        self.processes.lock().await.insert(task_id.to_string(), pid);

        Ok(pid.to_string())
    }

    async fn stop(&self, task_id: &str) -> Result<(), PluginError> {
        let pid = self
            .processes
            .lock()
            .await
            .remove(task_id)
            .ok_or_else(|| PluginError::NotFound(format!("No process for task {}", task_id)))?;

        debug!("ProcessRuntime stopping task {} (PID {})", task_id, pid);

        // Send SIGTERM via kill command.
        let result = tokio::process::Command::new("kill")
            .arg(pid.to_string())
            .output()
            .await;

        match result {
            Ok(_) => {
                info!("ProcessRuntime stopped task {} (PID {})", task_id, pid);
                Ok(())
            }
            Err(e) => {
                error!("Failed to stop PID {}: {}", pid, e);
                Err(PluginError::Execution(format!(
                    "Failed to kill PID {}: {}",
                    pid, e
                )))
            }
        }
    }

    async fn is_running(&self, task_id: &str) -> bool {
        let processes = self.processes.lock().await;
        let Some(pid) = processes.get(task_id) else {
            return false;
        };

        // `kill -0` checks process existence without sending a signal.
        match tokio::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .await
        {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    async fn output(&self, task_id: &str) -> Result<Option<String>, PluginError> {
        let path = self.output_path(task_id);
        if !path.exists() {
            return Ok(None);
        }

        tokio::fs::read_to_string(&path)
            .await
            .map(Some)
            .map_err(|e| {
                PluginError::Execution(format!(
                    "Failed to read output file {}: {}",
                    path.display(),
                    e
                ))
            })
    }
}

// ---------------------------------------------------------------------------
// TmuxRuntime -- tmux session-based isolation
// ---------------------------------------------------------------------------

/// Manages agent processes inside isolated tmux sessions.
pub struct TmuxRuntime {
    sessions: Arc<Mutex<HashMap<String, String>>>,
    socket_path: Option<String>,
}

impl TmuxRuntime {
    /// Create a new `TmuxRuntime` using the default tmux socket.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            socket_path: None,
        }
    }

    /// Create a new `TmuxRuntime` with a custom tmux socket path.
    pub fn with_socket(socket_path: String) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            socket_path: Some(socket_path),
        }
    }

    /// Derive a deterministic tmux session name for a task.
    fn session_name(task_id: &str) -> String {
        format!("d3vx-{}", task_id)
    }

    /// Build the base tmux command with optional socket path.
    fn tmux_base(&self) -> String {
        match &self.socket_path {
            Some(p) => format!("tmux -S {}", p),
            None => "tmux".into(),
        }
    }
}

impl Default for TmuxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TmuxRuntime {
    fn name(&self) -> &str {
        "tmux-runtime"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn description(&self) -> Option<&str> {
        Some("Tmux session-based runtime")
    }
    fn init(&self, _context: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}

impl crate::plugin::AdapterPlugin for TmuxRuntime {
    fn slot(&self) -> PluginSlot {
        PluginSlot::Runtime
    }
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            name: self.name().into(),
            version: self.version().into(),
            slot: self.slot(),
            description: self.description().unwrap_or_default().into(),
        }
    }
}

#[async_trait]
impl RuntimePlugin for TmuxRuntime {
    async fn start(&self, task_id: &str, command: &str) -> Result<String, PluginError> {
        let session = Self::session_name(task_id);
        let tmux = self.tmux_base();

        debug!(
            "TmuxRuntime starting task {} in session {}",
            task_id, session
        );

        let create_cmd = format!("{} new-session -d -s {}", tmux, session);
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&create_cmd)
            .output()
            .await
            .map_err(|e| PluginError::Execution(format!("Failed to create tmux session: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PluginError::Execution(format!(
                "tmux new-session failed: {}",
                stderr.trim()
            )));
        }

        // Send the actual command into the session.
        let send_cmd = format!("{} send-keys -t {} '{}' Enter", tmux, session, command);
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&send_cmd)
            .output()
            .await
            .map_err(|e| {
                PluginError::Execution(format!("Failed to send command to tmux: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("tmux send-keys warning: {}", stderr.trim());
        }

        info!(
            "TmuxRuntime started task {} in session {}",
            task_id, session
        );

        self.sessions
            .lock()
            .await
            .insert(task_id.to_string(), session.clone());

        Ok(session)
    }

    async fn stop(&self, task_id: &str) -> Result<(), PluginError> {
        let session = self.sessions.lock().await.remove(task_id).ok_or_else(|| {
            PluginError::NotFound(format!("No tmux session for task {}", task_id))
        })?;

        let tmux = self.tmux_base();
        debug!("TmuxRuntime stopping session {}", session);

        let kill_cmd = format!("{} kill-session -t {}", tmux, session);
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&kill_cmd)
            .output()
            .await
            .map_err(|e| PluginError::Execution(format!("Failed to kill tmux session: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PluginError::Execution(format!(
                "tmux kill-session failed: {}",
                stderr.trim()
            )));
        }

        info!("TmuxRuntime stopped session {}", session);
        Ok(())
    }

    async fn is_running(&self, task_id: &str) -> bool {
        let sessions = self.sessions.lock().await;
        let Some(session) = sessions.get(task_id) else {
            return false;
        };

        let tmux = self.tmux_base();
        let check_cmd = format!("{} has-session -t {}", tmux, session);

        match tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&check_cmd)
            .output()
            .await
        {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    async fn output(&self, task_id: &str) -> Result<Option<String>, PluginError> {
        let sessions = self.sessions.lock().await;
        let Some(session) = sessions.get(task_id) else {
            return Ok(None);
        };

        let tmux = self.tmux_base();
        let capture_cmd = format!("{} capture-pane -t {} -p", tmux, session);

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&capture_cmd)
            .output()
            .await
            .map_err(|e| PluginError::Execution(format!("Failed to capture tmux pane: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(if text.is_empty() { None } else { Some(text) })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::slots::AdapterPlugin;

    #[test]
    fn test_process_runtime_descriptor() {
        let rt = ProcessRuntime::new();
        let desc = rt.descriptor();
        assert_eq!(desc.name, "process-runtime");
        assert_eq!(desc.slot, PluginSlot::Runtime);
    }

    #[test]
    fn test_tmux_runtime_descriptor() {
        let rt = TmuxRuntime::new();
        let desc = rt.descriptor();
        assert_eq!(desc.name, "tmux-runtime");
        assert_eq!(desc.slot, PluginSlot::Runtime);
    }

    #[test]
    fn test_session_name() {
        assert_eq!(TmuxRuntime::session_name("abc-123"), "d3vx-abc-123");
    }

    #[test]
    fn test_tmux_base_default() {
        let rt = TmuxRuntime::new();
        assert_eq!(rt.tmux_base(), "tmux");
    }

    #[test]
    fn test_tmux_base_custom_socket() {
        let rt = TmuxRuntime::with_socket("/tmp/custom.sock".into());
        assert_eq!(rt.tmux_base(), "tmux -S /tmp/custom.sock");
    }

    #[test]
    fn test_output_path() {
        let rt = ProcessRuntime::with_output_dir(PathBuf::from("/tmp/d3vx-test"));
        let path = rt.output_path("task-42");
        assert!(path.ends_with("d3vx-task-42.out"));
    }

    #[test]
    fn test_process_runtime_default() {
        let rt = ProcessRuntime::default();
        assert_eq!(rt.descriptor().name, "process-runtime");
    }

    #[test]
    fn test_tmux_runtime_default() {
        let rt = TmuxRuntime::default();
        assert_eq!(rt.descriptor().name, "tmux-runtime");
    }

    #[tokio::test]
    async fn test_process_runtime_not_found() {
        let rt = ProcessRuntime::new();
        let result = rt.stop("nonexistent").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_tmux_runtime_not_found() {
        let rt = TmuxRuntime::new();
        let result = rt.stop("nonexistent").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_process_runtime_not_running() {
        let rt = ProcessRuntime::new();
        assert!(!rt.is_running("nonexistent").await);
    }

    #[tokio::test]
    async fn test_tmux_runtime_not_running() {
        let rt = TmuxRuntime::new();
        assert!(!rt.is_running("nonexistent").await);
    }

    #[tokio::test]
    async fn test_process_runtime_no_output() {
        let rt = ProcessRuntime::new();
        let result = rt.output("nonexistent").await.unwrap();
        assert!(result.is_none());
    }
}
