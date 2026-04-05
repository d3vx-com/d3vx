//! IPC Client - Communication with the d3vx agent
//!
//! Handles JSON-RPC communication over stdin/stdout with the
//! Node.js agent process.

use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::protocol::{jsonrpc, Event, Method};
use super::types::{Message, PermissionRequest, ThinkingState, TokenUsage};

// ────────────────────────────────────────────────────────────
// IPC Client
// ────────────────────────────────────────────────────────────

/// IPC Client for communicating with the agent
#[derive(Clone)]
pub struct IpcClient {
    /// Next request ID
    next_id: Arc<AtomicU64>,
    /// Request sender
    request_tx: mpsc::Sender<String>,
    /// Response receiver (note: cloning will share the receiver)
    response_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<jsonrpc::Response>>>,
    /// Event receiver
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<(Event, serde_json::Value)>>>,
}

impl IpcClient {
    /// Create a new IPC client
    pub fn new() -> (Self, IpcHandle) {
        let (request_tx, request_rx) = mpsc::channel(100);
        let (response_tx, response_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Spawn IO task
        tokio::spawn(Self::io_task(
            request_rx,
            response_tx,
            event_tx,
            shutdown_rx,
        ));

        let client = Self {
            next_id: Arc::new(AtomicU64::new(1)),
            request_tx,
            response_rx: Arc::new(tokio::sync::Mutex::new(response_rx)),
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
        };

        let handle = IpcHandle {
            shutdown: shutdown_tx,
        };

        (client, handle)
    }

    /// Send a request and wait for response
    pub async fn send_request<T: serde::Serialize>(
        &self,
        method: Method,
        params: Option<T>,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut req = jsonrpc::Request::new(id, method);
        if let Some(p) = params {
            req = req.with_params(p);
        }

        let json = serde_json::to_string(&req)?;
        debug!("Sending request: {}", json);

        self.request_tx.send(json).await?;

        // Wait for response
        let mut rx = self.response_rx.lock().await;
        let response = rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("IPC channel closed"))?;

        if let Some(err) = response.error {
            anyhow::bail!("IPC error: {} (code: {})", err.message, err.code);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<(Event, serde_json::Value)> {
        // Try to lock and receive without blocking
        if let Ok(mut rx) = self.event_rx.try_lock() {
            rx.try_recv().ok()
        } else {
            None
        }
    }

    /// Receive an event (blocking)
    pub async fn recv_event(&self) -> Option<(Event, serde_json::Value)> {
        let mut rx = self.event_rx.lock().await;
        rx.recv().await
    }

    /// IO task that handles stdin/stdout
    async fn io_task(
        mut request_rx: mpsc::Receiver<String>,
        response_tx: mpsc::Sender<jsonrpc::Response>,
        event_tx: mpsc::Sender<(Event, serde_json::Value)>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        info!("IPC client started");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();
        let mut writer = stdout;

        loop {
            tokio::select! {
                // Check for shutdown
                _ = shutdown_rx.recv() => {
                    info!("IPC client shutting down");
                    break;
                }

                // Send outgoing requests
                Some(json) = request_rx.recv() => {
                    let json = format!("{}\n", json);
                    if let Err(e) = writer.write_all(json.as_bytes()).await {
                        error!("Failed to write to stdout: {}", e);
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!("Failed to flush stdout: {}", e);
                        break;
                    }
                }

                // Read incoming messages
                result = reader.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            // Parse as response or notification
                            if let Ok(response) = serde_json::from_str::<jsonrpc::Response>(&line) {
                                if response_tx.send(response).await.is_err() {
                                    break;
                                }
                            } else if let Ok(notification) = serde_json::from_str::<serde_json::Value>(&line) {
                                // Parse notification manually to avoid lifetime issues
                                if let Some(method) = notification.get("method").and_then(|m| m.as_str()) {
                                    if let Ok(event) = method.parse::<Event>() {
                                        let params = notification.get("params").cloned().unwrap_or(serde_json::Value::Null);
                                        if event_tx.send((event, params)).await.is_err() {
                                            break;
                                        }
                                    } else {
                                        warn!("Unknown notification method: {}", method);
                                    }
                                }
                            } else {
                                warn!("Failed to parse IPC message: {}", line);
                            }
                        }
                        Ok(None) => {
                            // EOF
                            info!("IPC stdin closed");
                            break;
                        }
                        Err(e) => {
                            error!("Failed to read from stdin: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Handle to shut down IPC client
pub struct IpcHandle {
    shutdown: mpsc::Sender<()>,
}

impl IpcHandle {
    /// Shut down the IPC client
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown.send(()).await?;
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────
// High-level API
// ────────────────────────────────────────────────────────────

impl IpcClient {
    /// Send a chat message
    pub async fn send_message(&self, content: &str) -> Result<()> {
        self.send_request(
            Method::SendMessage,
            Some(serde_json::json!({
                "content": content
            })),
        )
        .await?;
        Ok(())
    }

    /// Cancel current operation
    pub async fn cancel_current(&self) -> Result<()> {
        self.send_request::<()>(Method::CancelCurrent, None).await?;
        Ok(())
    }

    /// Clear chat history
    pub async fn clear_history(&self) -> Result<()> {
        self.send_request::<()>(Method::ClearHistory, None).await?;
        Ok(())
    }

    /// Set verbose mode
    pub async fn set_verbose(&self, verbose: bool) -> Result<()> {
        self.send_request(
            Method::SetVerbose,
            Some(serde_json::json!({
                "verbose": verbose
            })),
        )
        .await?;
        Ok(())
    }

    /// Respond to permission request
    pub async fn respond_permission(&self, request_id: &str, response: &str) -> Result<()> {
        self.send_request(
            Method::RespondPermission,
            Some(serde_json::json!({
                "requestId": request_id,
                "response": response
            })),
        )
        .await?;
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────
// Event Parsing
// ────────────────────────────────────────────────────────────

/// Parse an event into a typed value
pub fn parse_event(event: Event, value: serde_json::Value) -> Result<IpcEvent> {
    match event {
        Event::OnMessage | Event::OnMessageUpdate => {
            let msg: Message = serde_json::from_value(value)?;
            Ok(IpcEvent::Message(msg))
        }
        Event::OnToolCall | Event::OnToolCallUpdate => {
            let tc = serde_json::from_value(value)?;
            Ok(IpcEvent::ToolCall(tc))
        }
        Event::OnThinking => {
            let state: ThinkingState = serde_json::from_value(value)?;
            Ok(IpcEvent::Thinking(state))
        }
        Event::OnPermissionRequest => {
            let req: PermissionRequest = serde_json::from_value(value)?;
            Ok(IpcEvent::PermissionRequest(req))
        }
        Event::OnError => {
            let err: super::protocol::ErrorParams = serde_json::from_value(value)?;
            Ok(IpcEvent::Error(err.message))
        }
        Event::OnSessionEnd => {
            let usage: TokenUsage = serde_json::from_value(value)?;
            Ok(IpcEvent::SessionEnd(usage))
        }
    }
}

/// Typed IPC events
#[derive(Debug)]
pub enum IpcEvent {
    Message(Message),
    ToolCall(super::types::ToolCall),
    Thinking(ThinkingState),
    PermissionRequest(PermissionRequest),
    Error(String),
    SessionEnd(TokenUsage),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ipc_client_creation() {
        let (_client, _handle) = IpcClient::new();
    }
}
