//! MCP Manager
//!
//! Orchestrates multiple MCP server instances and provides a unified interface
//! for the application to interact with all configured MCP servers.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::client::McpClient;
use super::protocol::{CallToolResult, ListToolsResult, McpToolDefinition};
use crate::config::types::McpServer;

pub struct McpManager {
    clients: Arc<Mutex<HashMap<String, Arc<McpClient>>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn and initialize an MCP server from configuration
    pub async fn add_server(&self, name: String, config: McpServer) -> Result<()> {
        let client = Arc::new(McpClient::new(name.clone()));

        // Start the server process
        client
            .start(
                &config.command,
                &config.args,
                config.env.as_ref(),
                config.cwd.as_deref(),
            )
            .await?;

        // Initialize the server
        client
            .initialize()
            .await
            .context(format!("Failed to initialize MCP server '{}'", name))?;

        info!("MCP server '{}' initialized successfully", name);
        self.clients.lock().await.insert(name, client);
        Ok(())
    }

    /// Aggregate all tools from all active MCP servers
    pub async fn list_all_tools(&self) -> Vec<(String, McpToolDefinition)> {
        let clients = self.clients.lock().await;
        let mut all_tools = Vec::new();

        for (name, client) in clients.iter() {
            match client.call("tools/list", serde_json::json!({})).await {
                Ok(result) => {
                    if let Ok(list_result) = serde_json::from_value::<ListToolsResult>(result) {
                        for tool in list_result.tools {
                            all_tools.push((name.clone(), tool));
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools for MCP server '{}': {}", name, e);
                }
            }
        }

        all_tools
    }

    /// Execute a tool on a specific MCP server
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        let clients = self.clients.lock().await;
        let client = clients
            .get(server_name)
            .context(format!("MCP server '{}' not found", server_name))?;

        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });

        let result = client.call("tools/call", params).await?;
        let call_result: CallToolResult = serde_json::from_value(result)?;

        Ok(call_result)
    }

    pub async fn list_resources(
        &self,
        server_name: &str,
    ) -> Result<Vec<super::protocol::Resource>> {
        let clients = self.clients.lock().await;
        let client = clients.get(server_name).context("MCP server not found")?;
        let result = client.call("resources/list", serde_json::json!({})).await?;
        let list: super::protocol::ListResourcesResult = serde_json::from_value(result)?;
        Ok(list.resources)
    }

    pub async fn read_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> Result<super::protocol::ReadResourceResult> {
        let clients = self.clients.lock().await;
        let client = clients.get(server_name).context("MCP server not found")?;
        let result = client
            .call("resources/read", serde_json::json!({"uri": uri}))
            .await?;
        serde_json::from_value(result).context("Failed to parse resource result")
    }

    pub async fn list_prompts(&self, server_name: &str) -> Result<Vec<super::protocol::Prompt>> {
        let clients = self.clients.lock().await;
        let client = clients.get(server_name).context("MCP server not found")?;
        let result = client.call("prompts/list", serde_json::json!({})).await?;
        let list: super::protocol::ListPromptsResult = serde_json::from_value(result)?;
        Ok(list.prompts)
    }

    pub async fn shutdown_all(&self) {
        let mut clients = self.clients.lock().await;
        for (name, client) in clients.drain() {
            info!("Shutting down MCP server '{}'...", name);
            let _ = client.shutdown().await;
        }
    }
}
