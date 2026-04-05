//! LSP Bridge — incremental diagnostics for agents
//!
//! Provides a lightweight, async LSP bridge for per-file diagnostics.
//! This is the FAST path — the agent gets errors in milliseconds
//! rather than waiting for a full cargo check/tsc build.
//!
//! Design:
//! - One LSP server per configured language (rust-analyzer, typescript, etc.)
//! - Servers are started lazily (on first file edit of that type)
//! - Diagnostics are pushed via the standard LSP notification handler
//! - Query by file path to get cached diagnostics instantly

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// A single diagnostic from the LSP server.
#[derive(Debug, Clone)]
pub struct LspDiagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: LspSeverity,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspSeverity {
    Hint,
    Information,
    Warning,
    Error,
}

impl std::fmt::Display for LspSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LspSeverity::Hint => write!(f, "hint"),
            LspSeverity::Information => write!(f, "info"),
            LspSeverity::Warning => write!(f, "warning"),
            LspSeverity::Error => write!(f, "error"),
        }
    }
}

/// Configuration for a single LSP server.
#[derive(Debug, Clone)]
pub struct LspBridgeConfig {
    pub binary: String,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
}

/// LSP Bridge — manages servers and caches diagnostics.
pub struct LspBridge {
    servers: Arc<RwLock<HashMap<String, LspServerHandle>>>,
    configs: HashMap<String, LspBridgeConfig>,
    diagnostics: Arc<RwLock<HashMap<PathBuf, Vec<LspDiagnostic>>>>,
    root: PathBuf,
}

/// Handle to a running LSP server.
struct LspServerHandle {
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    request_id: Arc<Mutex<u64>>,
    _task: tokio::task::JoinHandle<()>,
}

impl LspBridge {
    /// Create a new bridge with server configs and project root.
    pub fn new(configs: Vec<LspBridgeConfig>, root: PathBuf) -> Self {
        let config_map: HashMap<_, _> = configs
            .into_iter()
            .enumerate()
            .map(|(i, c)| (format!("server-{}", i), c))
            .collect();
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            configs: config_map,
            diagnostics: Arc::new(RwLock::new(HashMap::new())),
            root,
        }
    }

    /// Get diagnostics for a file path.
    ///
    /// If the server for this file's language isn't started, it's
    /// launched lazily. Diagnostics are returned from the cache
    /// or fetched by sending a didOpen notification.
    pub async fn get_diagnostics(&self, path: &Path) -> Vec<LspDiagnostic> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let server_key = self.find_server_for_ext(ext).await;

        // If no server, try to start one
        if server_key.is_none() {
            return Vec::new();
        }
        let server_key = server_key.unwrap();

        // Check cached diagnostics first
        if let Some(cached) = self.diagnostics.read().await.get(path) {
            return cached.clone();
        }

        // Send didOpen to trigger diagnostics
        if let Some(handle) = self.servers.read().await.get(&server_key) {
            let _ = self.send_did_open(handle, path).await;
        }

        // Return whatever is cached now (maybe empty if server hasn't responded)
        self.diagnostics
            .read()
            .await
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    /// Format diagnostics as a compact string for agent consumption.
    pub fn format_for_agent(diagnostics: &[LspDiagnostic]) -> String {
        if diagnostics.is_empty() {
            return String::new();
        }

        let mut output = String::from("\n\n[LSP Diagnostics]\n");
        for d in diagnostics {
            if d.severity == LspSeverity::Hint {
                continue; // Skip hints, only show warnings and errors
            }
            output.push_str(&format!(
                "  {}:{}:{} [{}] {} ({})\n",
                d.file, d.line, d.column, d.severity, d.message, d.source
            ));
        }
        output
    }

    /// Find a server config matching the given extension.
    fn find_config_for_ext(&self, ext: &str) -> Option<(&str, &LspBridgeConfig)> {
        self.configs
            .iter()
            .find(|(_, c)| c.extensions.iter().any(|e| e.ends_with(ext)))
            .map(|(k, c)| (k.as_str(), c))
    }

    /// Find or start a server for the given extension.
    async fn find_server_for_ext(&self, ext: &str) -> Option<String> {
        // Check if already running
        let config_match = self.find_config_for_ext(ext)?;
        let (config_key, _config) = config_match;

        if self.servers.read().await.contains_key(config_key) {
            return Some(config_key.to_string());
        }

        // Start the server lazily
        self.start_server(config_key).await
    }

    /// Start an LSP server for a config key.
    async fn start_server(&self, config_key: &str) -> Option<String> {
        let config = self.configs.get(config_key)?;

        info!(
            server = %config_key,
            binary = %config.binary,
            "Starting LSP server"
        );

        let mut cmd = Command::new(&config.binary)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .ok()?;

        let stdin = cmd.stdin.take()?;
        let stdout = cmd.stdout.take()?;

        let handle = LspServerHandle {
            stdin: Arc::new(Mutex::new(Some(stdin))),
            request_id: Arc::new(Mutex::new(1)),
            _task: tokio::spawn(Self::reader_loop(
                stdout,
                self.diagnostics.clone(),
                self.root.clone(),
            )),
        };

        // Send initialize
        let id = {
            let mut id_guard = handle.request_id.lock().await;
            let current = *id_guard;
            *id_guard = current + 1;
            current
        };

        let init_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "capabilities": {
                    "textDocument": {
                        "publishDiagnostics": {
                            "relatedInformation": false
                        }
                    }
                },
                "rootUri": path_to_uri(&self.root),
            }
        });

        if let Some(stdin_ref) = handle.stdin.lock().await.as_mut() {
            let _ = send_jsonrpc(stdin_ref, &init_msg).await;
        }

        self.servers
            .write()
            .await
            .insert(config_key.to_string(), handle);

        Some(config_key.to_string())
    }

    /// Send textDocument/didOpen to notify the server about a file.
    async fn send_did_open(&self, handle: &LspServerHandle, path: &Path) -> std::io::Result<()> {
        let content = tokio::fs::read_to_string(path).await.unwrap_or_default();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": path_to_uri(path),
                    "languageId": "rust",
                    "version": 1,
                    "text": content,
                }
            }
        });

        if let Some(stdin_ref) = handle.stdin.lock().await.as_mut() {
            send_jsonrpc(stdin_ref, &msg).await
        } else {
            Ok(())
        }
    }

    /// Background reader loop: parses LSP notifications from stdout.
    async fn reader_loop(
        stdout: tokio::process::ChildStdout,
        diagnostics: Arc<RwLock<HashMap<PathBuf, Vec<LspDiagnostic>>>>,
        root: PathBuf,
    ) {
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

            if !line.trim().is_empty() && content_length > 0 {
                let mut buf = vec![0u8; content_length];
                if reader.read_exact(&mut buf).await.is_ok() {
                    let msg = String::from_utf8_lossy(&buf).to_string();
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&msg) {
                        Self::handle_lsp_message(&value, &diagnostics, &root).await;
                    }
                }
                content_length = 0;
            }
        }

        warn!("LSP server stdout closed, reader loop exiting");
    }

    /// Parse an incoming LSP message and cache diagnostics.
    async fn handle_lsp_message(
        value: &serde_json::Value,
        diagnostics: &Arc<RwLock<HashMap<PathBuf, Vec<LspDiagnostic>>>>,
        root: &Path,
    ) {
        if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
            if method != "textDocument/publishDiagnostics" {
                return;
            }

            let params = match value.get("params") {
                Some(p) => p,
                None => return,
            };

            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");

            let diags = params.get("diagnostics").and_then(|d| d.as_array());
            if diags.is_none() {
                return;
            }

            let path = uri_to_path(uri, root);
            if path.is_none() {
                return;
            }
            let path = path.unwrap();

            let parsed: Vec<LspDiagnostic> = diags
                .unwrap()
                .iter()
                .filter_map(|d| {
                    let severity_num = d.get("severity").and_then(|s| s.as_u64()).unwrap_or(3);
                    let severity = match severity_num {
                        1 => LspSeverity::Error,
                        2 => LspSeverity::Warning,
                        3 => LspSeverity::Information,
                        4 => LspSeverity::Hint,
                        _ => LspSeverity::Warning,
                    };

                    let range = d.get("range")?;
                    let start = range.get("start")?;
                    let line = start.get("line")?.as_u64()? as u32;
                    let column = start.get("character")?.as_u64()? as u32;

                    Some(LspDiagnostic {
                        file: path.to_string_lossy().to_string(),
                        line,
                        column,
                        severity,
                        message: d.get("message")?.as_str()?.to_string(),
                        source: d
                            .get("source")
                            .and_then(|s| s.as_str())
                            .unwrap_or("LSP")
                            .to_string(),
                    })
                })
                .collect();

            debug!(
                file = ?path,
                count = parsed.len(),
                "Cached LSP diagnostics"
            );

            diagnostics.write().await.insert(path, parsed);
        }
    }
}

/// Convert a file path to a file:// URI.
fn path_to_uri(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    format!("file://{}", abs.to_string_lossy())
}

/// Convert a file:// URI back to a path.
fn uri_to_path(uri: &str, root: &Path) -> Option<PathBuf> {
    let stripped = uri.strip_prefix("file://")?;
    let path = Path::new(stripped);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        Some(root.join(path))
    }
}

/// Write a JSON-RPC message to the LSP stdin pipe.
async fn send_jsonrpc(
    stdin: &mut tokio::process::ChildStdin,
    msg: &serde_json::Value,
) -> std::io::Result<()> {
    let content = serde_json::to_string(msg).unwrap();
    let data = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
    stdin.write_all(data.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_empty() {
        let out = LspBridge::format_for_agent(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn test_format_skips_hints() {
        let diags = vec![LspDiagnostic {
            file: "src/main.rs".to_string(),
            line: 10,
            column: 5,
            severity: LspSeverity::Hint,
            message: "unused variable".into(),
            source: "rustc".into(),
        }];
        let out = LspBridge::format_for_agent(&diags);
        // Hints are skipped, so no diagnostic content
        assert!(
            !out.contains("unused variable"),
            "hints should be filtered out"
        );
    }

    #[test]
    fn test_format_shows_errors() {
        let diags = vec![LspDiagnostic {
            file: "src/main.rs".to_string(),
            line: 10,
            column: 5,
            severity: LspSeverity::Error,
            message: "cannot find value `x`".into(),
            source: "rustc".into(),
        }];
        let out = LspBridge::format_for_agent(&diags);
        assert!(out.contains("LSP Diagnostics"));
        assert!(out.contains("error"));
        assert!(out.contains("cannot find value `x`"));
        assert!(out.contains("src/main.rs:10:5"));
    }

    #[test]
    fn test_path_to_uri() {
        let uri = path_to_uri(Path::new("/tmp/test.rs"));
        assert!(uri.starts_with("file:///"));
        assert!(uri.ends_with("/tmp/test.rs"));
    }
}
