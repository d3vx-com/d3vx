//! MCP Client Reconnection and Multi-Server Tests
//!
//! Features covered:
//! - Basic initialization and normal requests
//! - Timeout handling
//! - Automatic reconnection after a server crash
//! - Multi-server coordination (McpManager tests)

#![cfg(test)]

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::time::Duration;

use crate::mcp::client::McpClient;
use crate::mcp::manager::McpManager;
// use crate::mcp::protocol::InitializeResult;

/// Returns the path to the script and a cleanup guard.
struct MockServer {
    script_path: PathBuf,
}

impl MockServer {
    fn new(script_content: &str) -> Self {
        let dir = env::temp_dir();
        let name = format!("mock_mcp_server_{}.py", uuid::Uuid::new_v4());
        let script_path = dir.join(name);
        fs::write(&script_path, script_content).unwrap();

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        Self { script_path }
    }

    fn path(&self) -> String {
        self.script_path.to_string_lossy().to_string()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.script_path);
    }
}

/// The python script content that simulates an MCP server supporting crash commands.
const MOCK_SERVER_SCRIPT: &str = r#"
import sys
import json
import time

def respond(id, result=None, error=None):
    res = {"jsonrpc": "2.0", "id": id}
    if result is not None:
        res["result"] = result
    if error is not None:
        res["error"] = error
    print(json.dumps(res), flush=True)

def handle_request(req):
    method = req.get("method")
    id = req.get("id")
    params = req.get("params", {})

    if method == "initialize":
        respond(id, result={
            "protocol_version": "2024-11-05",
            "capabilities": {},
            "server_info": {"name": "mock-server", "version": "1.0.0"}
        })
    elif method == "notifications/initialized":
        # Just a notification, no response
        pass
    elif method == "test/ping":
        respond(id, result={"pong": True})
    elif method == "test/crash":
        # Simulate an immediate crash before responding
        sys.exit(1)
    elif method == "test/timeout":
        # Simulate a timeout by sleeping longer than the client timeout
        time.sleep(10)
        respond(id, result={"pong": True})
    else:
        respond(id, error={"code": -32601, "message": f"Method not found: {method}"})

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
        handle_request(req)
    except Exception as e:
        print(json.dumps({"jsonrpc": "2.0", "error": {"code": -32700, "message": str(e)}}), flush=True)

"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_mcp_client_basic_communication() {
    let server = MockServer::new(MOCK_SERVER_SCRIPT);
    // Use python3 to run the script
    let client = McpClient::new("test-server".to_string());

    client
        .start("python3", &[server.path()], None, None)
        .await
        .unwrap();
    assert!(client.is_alive().await);

    // Initialize
    let init_result = client.initialize().await.unwrap();
    assert_eq!(init_result.server_info.name, "mock-server");
    assert_eq!(init_result.protocol_version, "2024-11-05");

    // Normal request
    let res = client
        .call("test/ping", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(res["pong"].as_bool(), Some(true));

    // Shutdown
    client.shutdown().await.unwrap();
    assert!(!client.is_alive().await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_mcp_client_timeout() {
    let server = MockServer::new(MOCK_SERVER_SCRIPT);
    // Set a timeout of 3 seconds (gives python enough time to boot for initialize)
    let client = McpClient::with_timeout("timeout-server".to_string(), 3);

    client
        .start("python3", &[server.path()], None, None)
        .await
        .unwrap();
    client.initialize().await.unwrap();

    // The script sleeps for 5s on this endpoint
    let err = client
        .call("test/timeout", serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("timed out after 3s"));

    client.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_mcp_client_reconnection() {
    let server = MockServer::new(MOCK_SERVER_SCRIPT);
    let client = McpClient::with_timeout("reconnect-server".to_string(), 5);

    client
        .start("python3", &[server.path()], None, None)
        .await
        .unwrap();
    client.initialize().await.unwrap();

    // 1. Initial ping succeeds
    let res1 = client
        .call("test/ping", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(res1["pong"].as_bool(), Some(true));

    // 2. Trigger a crash (this call itself will fail because the python process exits immediately)
    let err = client
        .call("test/crash", serde_json::json!({}))
        .await
        .unwrap_err();
    // The server dies before sending a response, so the channel is closed
    assert!(
        err.to_string().contains("channel closed")
            || err.to_string().contains("timed out")
            || err.to_string().contains("not connected")
            || err.to_string().contains("is unrecoverable")
    );

    // Ensure the client knows the server is dead
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!client.is_alive().await);

    // 3. Make another request. This should trigger automatic reconnection under the hood
    //    and then succeed against the newly spawned Python process.
    let res2 = client
        .call("test/ping", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(res2["pong"].as_bool(), Some(true));

    // Server should be alive again
    assert!(client.is_alive().await);

    client.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_manager_multi_server() {
    let server1 = MockServer::new(MOCK_SERVER_SCRIPT);
    let server2 = MockServer::new(MOCK_SERVER_SCRIPT);

    let manager = McpManager::new();

    manager
        .add_server(
            "s1".to_string(),
            crate::config::types::McpServer {
                command: "python3".to_string(),
                args: vec![server1.path()],
                env: None,
                cwd: None,
                timeout_ms: Some(5000),
            },
        )
        .await
        .unwrap();

    manager
        .add_server(
            "s2".to_string(),
            crate::config::types::McpServer {
                command: "python3".to_string(),
                args: vec![server2.path()],
                env: None,
                cwd: None,
                timeout_ms: None,
            },
        )
        .await
        .unwrap();

    // Both servers should have tools (though our mock script returns empty capabilities)
    // We'll just test that we can dispatch a direct call via the manager using `call_tool`.
    // We haven't implemented tool handling in the Python script, so we'll get a Method not found error,
    // which proves the internal client got the message and parsed the error response!

    let err1 = manager
        .call_tool("s1", "fake-tool", serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err1.to_string().contains("Method not found: tools/call"));

    let err2 = manager
        .call_tool("s2", "fake-tool", serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err2.to_string().contains("Method not found: tools/call"));

    manager.shutdown_all().await;
}
