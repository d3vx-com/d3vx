//! MCP Client Implementation
//!
//! Handles the lifecycle of a single MCP server process and provides
//! an async interface for calling tools and managing state.
//!
//! ## Hardening
//!
//! - Configurable request timeout.
//! - Automatic reconnection on server crash (up to `MAX_RECONNECT_ATTEMPTS`).
//! - `Drop` guard to ensure child processes are killed on cleanup.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info, warn};

use super::protocol::{
    ClientInfo, InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse,
};

/// Maximum number of automatic reconnection attempts.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// Default request timeout in seconds.
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct McpClient {
    pub name: String,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<tokio::process::ChildStdin>>,
    next_id: Mutex<u64>,
    pending_requests: Arc<Mutex<HashMap<serde_json::Value, oneshot::Sender<JsonRpcResponse>>>>,
    /// Stored spawn config for reconnection.
    spawn_config: Mutex<Option<SpawnConfig>>,
    /// Configurable request timeout.
    request_timeout_secs: u64,
}

/// Internal struct to store the server spawn configuration for reconnection.
#[derive(Clone)]
struct SpawnConfig {
    command: String,
    args: Vec<String>,
    env: Option<HashMap<String, String>>,
    cwd: Option<String>,
}

impl McpClient {
    /// Create a new MCP client (doesn't start the process).
    pub fn new(name: String) -> Self {
        Self {
            name,
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            next_id: Mutex::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            spawn_config: Mutex::new(None),
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
        }
    }

    /// Create a new MCP client with a custom request timeout.
    pub fn with_timeout(name: String, timeout_secs: u64) -> Self {
        let mut client = Self::new(name);
        client.request_timeout_secs = timeout_secs;
        client
    }

    /// Start the MCP server process.
    pub async fn start(
        &self,
        command: &str,
        args: &[String],
        env: Option<&HashMap<String, String>>,
        cwd: Option<&str>,
    ) -> Result<()> {
        // Store spawn config for reconnection
        *self.spawn_config.lock().await = Some(SpawnConfig {
            command: command.to_string(),
            args: args.to_vec(),
            env: env.cloned(),
            cwd: cwd.map(|s| s.to_string()),
        });

        self.spawn_internal(command, args, env, cwd).await
    }

    /// Internal spawn logic (shared between `start` and `reconnect`).
    async fn spawn_internal(
        &self,
        command: &str,
        args: &[String],
        env: Option<&HashMap<String, String>>,
        cwd: Option<&str>,
    ) -> Result<()> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit()); // Pass through stderr for debugging
                                      // Ensure the child process is killed when the parent process exits
        cmd.kill_on_drop(true);

        #[cfg(unix)]
        {
            // Create a new process group for the child to allow killing the whole group
            unsafe {
                cmd.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
        }

        if let Some(env_vars) = env {
            cmd.envs(env_vars);
        }

        if let Some(working_dir) = cwd {
            cmd.current_dir(working_dir);
        }

        info!(
            "Starting MCP server '{}' with command: {} {}",
            self.name,
            command,
            args.join(" ")
        );
        let mut child = cmd.spawn().context("Failed to spawn MCP server")?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to take stdin from MCP process")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to take stdout from MCP process")?;

        *self.child.lock().await = Some(child);
        *self.stdin.lock().await = Some(stdin);

        // Start background stdout reader
        let pending_requests = self.pending_requests.clone();
        let server_name = self.name.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();

            while let Ok(Some(line)) = reader.next_line().await {
                debug!("MCP '{}' -> {}", server_name, line);

                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    let mut requests = pending_requests.lock().await;
                    if let Some(tx) = requests.remove(&response.id) {
                        let _ = tx.send(response);
                    }
                } else {
                    // Could be a notification or a malformed message
                    debug!(
                        "MCP '{}' received non-response or malformed line: {}",
                        server_name, line
                    );
                }
            }
            warn!("MCP server '{}' stdout closed", server_name);
        });

        Ok(())
    }

    /// Check if the MCP server process is still running.
    pub async fn is_alive(&self) -> bool {
        let mut guard = self.child.lock().await;
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => true,     // Still running
                Ok(Some(_)) => false, // Exited
                Err(_) => false,      // Error checking status
            }
        } else {
            false
        }
    }

    /// Attempt to reconnect to a crashed MCP server.
    async fn reconnect(&self) -> Result<()> {
        let config = {
            let guard = self.spawn_config.lock().await;
            guard
                .clone()
                .context("No spawn config stored for reconnection")?
        };

        // Clean up old process
        self.shutdown().await.ok();

        // Wait a moment before reconnecting
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        self.spawn_internal(
            &config.command,
            &config.args,
            config.env.as_ref(),
            config.cwd.as_deref(),
        )
        .await?;

        // Re-initialize the server
        self.initialize().await?;

        info!("MCP server '{}' reconnected successfully", self.name);
        Ok(())
    }

    /// Send a JSON-RPC request and wait for the response.
    /// Automatically attempts reconnection if the server has crashed.
    pub async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        // Try the call, and if the server is dead, attempt reconnection
        match self.call_internal(method, params.clone()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // Check if the server is dead
                if !self.is_alive().await {
                    warn!(
                        "MCP server '{}' appears dead, attempting reconnection...",
                        self.name
                    );
                    for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
                        match self.reconnect().await {
                            Ok(_) => {
                                info!(
                                    "MCP server '{}' reconnected (attempt {})",
                                    self.name, attempt
                                );
                                // Retry the original call
                                return self.call_internal(method, params).await;
                            }
                            Err(reconnect_err) => {
                                error!(
                                    "MCP server '{}' reconnect attempt {} failed: {}",
                                    self.name, attempt, reconnect_err
                                );
                                if attempt < MAX_RECONNECT_ATTEMPTS {
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        500 * attempt as u64,
                                    ))
                                    .await;
                                }
                            }
                        }
                    }
                    Err(anyhow::anyhow!(
                        "MCP server '{}' is unrecoverable after {} reconnection attempts: {}",
                        self.name,
                        MAX_RECONNECT_ATTEMPTS,
                        e
                    ))
                } else {
                    Err(e) // Server is alive but call failed for another reason
                }
            }
        }
    }

    /// Internal call without reconnection logic.
    async fn call_internal(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = {
            let mut id_guard = self.next_id.lock().await;
            let id = *id_guard;
            *id_guard += 1;
            id
        };
        let id_val = serde_json::Value::Number(id.into());

        let request = JsonRpcRequest::new(id_val.clone(), method, params);
        let message = serde_json::to_string(&request)? + "\n";

        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().await.insert(id_val, tx);

        {
            let mut stdin_guard = self.stdin.lock().await;
            if let Some(ref mut stdin) = *stdin_guard {
                stdin.write_all(message.as_bytes()).await?;
                stdin.flush().await?;
            } else {
                return Err(anyhow::anyhow!(
                    "MCP client '{}' is not connected",
                    self.name
                ));
            }
        }

        // Wait for response with configurable timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(self.request_timeout_secs),
            rx,
        )
        .await
        .context(format!(
            "MCP request to '{}' timed out after {}s",
            self.name, self.request_timeout_secs
        ))??;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!(
                "MCP Error ({}): {}",
                error.code,
                error.message
            ));
        }

        response.result.context("MCP response missing result")
    }

    /// Initialize the MCP server.
    pub async fn initialize(&self) -> Result<InitializeResult> {
        let params = serde_json::to_value(InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({}),
            client_info: ClientInfo {
                name: "d3vx".to_string(),
                version: "0.1.0".to_string(),
            },
        })?;

        let result = self.call_internal("initialize", params).await?;
        let init_result: InitializeResult = serde_json::from_value(result)?;

        // Send 'notifications/initialized'
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let message = serde_json::to_string(&notification)? + "\n";

        let mut stdin_guard = self.stdin.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        Ok(init_result)
    }

    /// Gracefully shut down the MCP server process.
    pub async fn shutdown(&self) -> Result<()> {
        // Drop stdin first to signal the server to close
        *self.stdin.lock().await = None;

        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            #[cfg(unix)]
            let pgid = child.id().map(|id| id as i32);

            // Give the server a moment to exit cleanly
            match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    info!("MCP server '{}' exited with status: {}", self.name, status);
                }
                Ok(Err(e)) => {
                    warn!("MCP server '{}' wait error: {}, killing", self.name, e);
                    self.kill_process(
                        &mut child,
                        #[cfg(unix)]
                        pgid,
                    )
                    .await;
                }
                Err(_) => {
                    // Timeout — force kill
                    warn!("MCP server '{}' did not exit in time, killing", self.name);
                    self.kill_process(
                        &mut child,
                        #[cfg(unix)]
                        pgid,
                    )
                    .await;
                }
            }
        }

        // Clear any remaining pending requests
        let mut pending = self.pending_requests.lock().await;
        pending.clear();

        Ok(())
    }

    /// Helper to kill a process and its process group on Unix.
    async fn kill_process(&self, child: &mut Child, #[cfg(unix)] pgid: Option<i32>) {
        #[cfg(unix)]
        if let Some(id) = pgid {
            debug!("Killing process group {}", id);
            // Killing -pgid sends the signal to the entire process group
            unsafe {
                libc::kill(-id, libc::SIGKILL);
            }
        }

        // Always try to kill the child directly as well
        let _ = child.kill().await;
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Best-effort synchronous kill to prevent leaked child processes.
        // The `kill_on_drop(true)` on Command handles this for tokio processes,
        // but this is an additional safety net.
        if let Ok(mut guard) = self.child.try_lock() {
            if let Some(ref mut child) = *guard {
                // Try to kill synchronously via the underlying std::process::Child
                // This is a no-op if the process has already exited.
                let _ = child.start_kill();
            }
        }
    }
}
