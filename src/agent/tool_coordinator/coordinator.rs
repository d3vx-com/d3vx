//! Tool coordinator implementation and builder

use super::types::{
    CoordinatorToolDefinition, ToolAdapter, ToolCoordinatorError, ToolExecutionResult, ToolHandler,
};
use crate::tools::{Tool, ToolContext, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Coordinates tool registration and execution.
///
/// The `ToolCoordinator` manages a registry of tools and provides
/// methods for registering, discovering, and executing tools.
pub struct ToolCoordinator {
    /// Registered tool handlers
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
    /// Default execution context
    default_context: ToolContext,
}

impl Default for ToolCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCoordinator {
    /// Create a new tool coordinator.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            default_context: ToolContext::default(),
        }
    }

    /// Create a coordinator with a custom default context.
    pub fn with_context(context: ToolContext) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            default_context: context,
        }
    }

    /// Register a tool handler.
    pub async fn register_handler(&self, handler: Arc<dyn ToolHandler>) {
        let definition = handler.definition();
        let name = definition.name.clone();

        let mut handlers = self.handlers.write().await;
        handlers.insert(name.clone(), handler);

        debug!(tool_name = %name, "Tool registered");
    }

    /// Register a tool implementing the Tool trait.
    pub async fn register_tool(&self, tool: impl Tool + 'static) {
        let adapter = ToolAdapter::new(Arc::new(tool));
        self.register_handler(Arc::new(adapter)).await;
    }

    /// Register multiple tools at once.
    pub async fn register_tools(&self, tools: Vec<Arc<dyn ToolHandler>>) {
        for tool in tools {
            self.register_handler(tool).await;
        }
    }

    /// Unregister a tool by name.
    pub async fn unregister(&self, name: &str) -> bool {
        let mut handlers = self.handlers.write().await;
        handlers.remove(name).is_some()
    }

    /// Check if a tool is registered.
    pub async fn has_tool(&self, name: &str) -> bool {
        let handlers = self.handlers.read().await;
        handlers.contains_key(name)
    }

    /// Get all registered tool definitions.
    pub async fn get_tool_definitions(&self) -> Vec<CoordinatorToolDefinition> {
        let handlers = self.handlers.read().await;
        handlers.values().map(|h| h.definition()).collect()
    }

    /// Get a specific tool definition by name.
    pub async fn get_tool_definition(&self, name: &str) -> Option<CoordinatorToolDefinition> {
        let handlers = self.handlers.read().await;
        handlers.get(name).map(|h| h.definition())
    }

    /// List all registered tool names.
    pub async fn list_tool_names(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }

    /// Get the number of registered tools.
    pub async fn tool_count(&self) -> usize {
        let handlers = self.handlers.read().await;
        handlers.len()
    }

    /// Execute a tool by name.
    pub async fn execute_tool(
        &self,
        name: &str,
        input: serde_json::Value,
        context: Option<&ToolContext>,
    ) -> Result<ToolResult, ToolCoordinatorError> {
        let handlers = self.handlers.read().await;

        let handler = handlers
            .get(name)
            .ok_or_else(|| ToolCoordinatorError::ToolNotFound(name.to_string()))?;

        let ctx = context.unwrap_or(&self.default_context);

        debug!(tool_name = %name, "Executing tool");

        let start = std::time::Instant::now();
        let result = handler.execute(input, ctx).await.map_err(|e| {
            warn!(tool_name = %name, error = %e, "Tool execution failed");
            ToolCoordinatorError::ExecutionFailed(e.to_string())
        })?;

        let elapsed = start.elapsed();

        debug!(
            tool_name = %name,
            elapsed_ms = elapsed.as_millis() as u64,
            is_error = result.is_error,
            "Tool execution completed"
        );

        Ok(result)
    }

    /// Execute a tool with timing metadata.
    pub async fn execute_tool_with_timing(
        &self,
        id: String,
        name: String,
        input: serde_json::Value,
        context: Option<&ToolContext>,
    ) -> ToolExecutionResult {
        let start = std::time::Instant::now();

        let result = match self.execute_tool(&name, input, context).await {
            Ok(r) => r,
            Err(e) => ToolResult::error(e.to_string()),
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        ToolExecutionResult::new(id, name, result, elapsed_ms)
    }

    /// Execute multiple tools concurrently.
    pub async fn execute_tools_concurrent(
        &self,
        calls: Vec<(String, String, serde_json::Value)>,
        context: Option<&ToolContext>,
    ) -> Vec<ToolExecutionResult> {
        use futures::future::join_all;

        let ctx = context
            .cloned()
            .unwrap_or_else(|| self.default_context.clone());

        let futures: Vec<_> = calls
            .into_iter()
            .map(|(id, name, input)| {
                let handlers = self.handlers.clone();
                let ctx = ctx.clone();

                async move {
                    let start = std::time::Instant::now();

                    let handlers_guard = handlers.read().await;
                    let result = if let Some(handler) = handlers_guard.get(&name) {
                        match handler.execute(input, &ctx).await {
                            Ok(r) => r,
                            Err(e) => ToolResult::error(e.to_string()),
                        }
                    } else {
                        ToolResult::error(format!("Tool not found: {}", name))
                    };

                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    ToolExecutionResult::new(id, name, result, elapsed_ms)
                }
            })
            .collect();

        join_all(futures).await
    }

    /// Clear all registered tools.
    pub async fn clear(&self) {
        let mut handlers = self.handlers.write().await;
        handlers.clear();
        debug!("All tools cleared");
    }

    /// Update the default execution context.
    pub fn set_default_context(&mut self, context: ToolContext) {
        self.default_context = context;
    }

    /// Get a reference to the default context.
    pub fn default_context(&self) -> &ToolContext {
        &self.default_context
    }
}

/// Builder for creating a tool coordinator with pre-registered tools.
pub struct ToolCoordinatorBuilder {
    handlers: Vec<Arc<dyn ToolHandler>>,
    context: ToolContext,
}

impl Default for ToolCoordinatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCoordinatorBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            context: ToolContext::default(),
        }
    }

    /// Set the default context.
    pub fn with_context(mut self, context: ToolContext) -> Self {
        self.context = context;
        self
    }

    /// Add a tool handler.
    pub fn with_handler(mut self, handler: Arc<dyn ToolHandler>) -> Self {
        self.handlers.push(handler);
        self
    }

    /// Add a tool implementing the Tool trait.
    pub fn with_tool(mut self, tool: impl Tool + 'static) -> Self {
        let adapter = ToolAdapter::new(Arc::new(tool));
        self.handlers.push(Arc::new(adapter));
        self
    }

    /// Build the coordinator.
    pub async fn build(self) -> ToolCoordinator {
        let coordinator = ToolCoordinator::with_context(self.context);
        coordinator.register_tools(self.handlers).await;
        coordinator
    }
}
