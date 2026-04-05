//! LSP Client Implementation

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};

use lsp_types::{ClientCapabilities, InitializeParams, ServerCapabilities};

pub struct LspClient {
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    request_id: Arc<RwLock<u64>>,
    capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    pending_requests: Arc<RwLock<HashMap<u64, oneshot::Sender<LspResponse>>>>,
}

impl LspClient {
    pub async fn start(binary: &str, args: &[String]) -> Result<Self, LspError> {
        info!("Starting LSP server: {} {:?}", binary, args);

        let mut cmd = Command::new(binary);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| LspError::StartFailed(e.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::IoError("Failed to get stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::IoError("Failed to get stdout".to_string()))?;

        let client = Self {
            stdin: Arc::new(Mutex::new(Some(stdin))),
            request_id: Arc::new(RwLock::new(1)),
            capabilities: Arc::new(RwLock::new(None)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        };

        client.spawn_reader(stdout).await;
        client.initialize().await?;
        Ok(client)
    }

    async fn spawn_reader(&self, stdout: tokio::process::ChildStdout) {
        let pending = self.pending_requests.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut content_length = 0usize;
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }

                if line.starts_with("Content-Length:") {
                    content_length = line
                        .trim_start_matches("Content-Length:")
                        .trim()
                        .parse()
                        .unwrap_or(0);
                    continue;
                }

                if line.trim().is_empty() && content_length > 0 {
                    let mut buf = vec![0u8; content_length];
                    if reader.read_exact(&mut buf).await.is_ok() {
                        let msg = String::from_utf8_lossy(&buf).to_string();
                        debug!("LSP response: {}", &msg[..msg.len().min(200)]);
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&msg) {
                            if let Some(id) = value.get("id").and_then(|i| i.as_u64()) {
                                if let Some(sender) = pending.write().await.remove(&id) {
                                    let _ = sender.send(LspResponse::Success(msg));
                                }
                            }
                        }
                    }
                    content_length = 0;
                }
            }
        });
    }

    async fn initialize(&self) -> Result<(), LspError> {
        let (tx, rx) = oneshot::channel();
        let id = self.next_id().await;
        self.pending_requests.write().await.insert(id, tx);

        let root_uri = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from));
        let params = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "capabilities": {},
                "processId": std::process::id(),
                "rootUri": root_uri,
            }
        });

        self.send_raw(&params).await?;
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| LspError::Timeout)?
            .map_err(|_| LspError::ChannelError)?;

        if let LspResponse::Success(resp) = response {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&resp) {
                if let Some(caps) = value.get("result").and_then(|r| r.get("capabilities")) {
                    if let Ok(c) = serde_json::from_value(caps.clone()) {
                        *self.capabilities.write().await = Some(c);
                    }
                }
            }
        }

        self.send_notification("initialized", serde_json::json!({}))
            .await?;
        info!("LSP server initialized");
        Ok(())
    }

    async fn next_id(&self) -> u64 {
        let mut id = self.request_id.write().await;
        let current = *id;
        *id = current + 1;
        current
    }

    async fn send_raw(&self, msg: &serde_json::Value) -> Result<(), LspError> {
        let content =
            serde_json::to_string(msg).map_err(|e| LspError::ParseError(e.to_string()))?;
        let data = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        let mut stdin = self.stdin.lock().await;
        if let Some(ref mut s) = *stdin {
            s.write_all(data.as_bytes())
                .await
                .map_err(|e| LspError::IoError(e.to_string()))?;
            s.flush()
                .await
                .map_err(|e| LspError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), LspError> {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        self.send_raw(&msg).await
    }

    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.capabilities.read().await.clone()
    }

    pub async fn shutdown(&self) -> Result<(), LspError> {
        self.send_notification("shutdown", serde_json::json!({}))
            .await?;
        self.send_notification("exit", serde_json::json!({}))
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum LspResponse {
    Success(String),
    Error { code: i32, message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("Failed to start server: {0}")]
    StartFailed(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Timeout waiting for response")]
    Timeout,
    #[error("Channel error")]
    ChannelError,
}
