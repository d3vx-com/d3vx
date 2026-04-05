//! Tool Registry
//!
//! Manages registration and execution of tools.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::tool_access::{AgentRole, ToolAccessValidator};
use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};
use tracing::error;

/// Global tool registry
static TOOL_REGISTRY: once_cell::sync::Lazy<RwLock<HashMap<String, Arc<dyn Tool>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

/// Register a tool
pub fn register_tool(tool: impl Tool + 'static) {
    let definition = tool.definition();
    let name = definition.name.clone();

    match TOOL_REGISTRY.write() {
        Ok(mut registry) => {
            registry.insert(name, Arc::new(tool));
        }
        Err(e) => {
            error!("Failed to acquire write lock on tool registry: {}", e);
        }
    }
}

/// Get a tool by name
pub fn get_tool(name: &str) -> Option<Arc<dyn Tool>> {
    TOOL_REGISTRY.read().ok()?.get(name).cloned()
}

/// Check if a tool exists
pub fn has_tool(name: &str) -> bool {
    TOOL_REGISTRY
        .read()
        .map(|r| r.contains_key(name))
        .unwrap_or(false)
}

/// List all registered tools
pub fn list_tools() -> Vec<Arc<dyn Tool>> {
    TOOL_REGISTRY
        .read()
        .map(|registry| registry.values().cloned().collect())
        .unwrap_or_default()
}

/// List all tool definitions
pub fn list_tool_definitions() -> Vec<ToolDefinition> {
    TOOL_REGISTRY
        .read()
        .map(|registry| registry.values().map(|t| t.definition()).collect())
        .unwrap_or_default()
}

/// List all tool names
pub fn list_tool_names() -> Vec<String> {
    let registry = TOOL_REGISTRY.read().unwrap();
    registry.keys().cloned().collect()
}

/// Unregister a tool
pub fn unregister_tool(name: &str) -> bool {
    let mut registry = TOOL_REGISTRY.write().unwrap();
    registry.remove(name).is_some()
}

/// Clear all tools
pub fn clear_tools() {
    let mut registry = TOOL_REGISTRY.write().unwrap();
    registry.clear();
}

/// Get the number of registered tools
pub fn tool_count() -> usize {
    let registry = TOOL_REGISTRY.read().unwrap();
    registry.len()
}

/// Execute a tool by name
pub async fn execute_tool(
    name: &str,
    input: serde_json::Value,
    context: &ToolContext,
) -> ToolResult {
    match get_tool(name) {
        Some(tool) => tool.execute(input, context).await,
        None => ToolResult::error(format!("Unknown tool: {}", name)),
    }
}

/// List tools filtered by role
pub fn list_tools_for_role(role: AgentRole) -> Vec<Arc<dyn Tool>> {
    let validator = ToolAccessValidator::new();
    let registry = TOOL_REGISTRY.read().unwrap();

    registry
        .values()
        .filter(|tool| {
            let name = tool.definition().name.clone();
            validator.is_allowed(role, &name)
        })
        .cloned()
        .collect()
}

/// List tool definitions filtered by role
pub fn list_tool_definitions_for_role(role: AgentRole) -> Vec<ToolDefinition> {
    let validator = ToolAccessValidator::new();
    let registry = TOOL_REGISTRY.read().unwrap();

    registry
        .values()
        .filter(|tool| {
            let name = tool.definition().name.clone();
            validator.is_allowed(role, &name)
        })
        .map(|t| t.definition())
        .collect()
}

/// List tool names filtered by role
pub fn list_tool_names_for_role(role: AgentRole) -> Vec<String> {
    let validator = ToolAccessValidator::new();
    let registry = TOOL_REGISTRY.read().unwrap();

    registry
        .keys()
        .filter(|name| validator.is_allowed(role, name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestTool;

    #[async_trait]
    impl Tool for TestTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "test".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            }
        }

        async fn execute(&self, _input: serde_json::Value, _context: &ToolContext) -> ToolResult {
            ToolResult::success("test output")
        }
    }

    #[test]
    fn test_register_and_get_tool() {
        clear_tools();

        register_tool(TestTool);

        assert!(has_tool("test"));
        assert!(!has_tool("nonexistent"));

        let tool = get_tool("test");
        assert!(tool.is_some());

        assert_eq!(tool_count(), 1);
    }

    #[test]
    fn test_list_tools() {
        clear_tools();

        register_tool(TestTool);

        let names = list_tool_names();
        assert_eq!(names, vec!["test"]);

        let defs = list_tool_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "test");
    }
}
