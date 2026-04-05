//! Tool coordinator tests

use super::*;
use crate::tools::{ToolContext, ToolResult};
use serde_json::json;
use std::sync::Arc;

/// Mock tool handler for testing
struct MockTool {
    name: String,
    result: ToolResult,
}

impl MockTool {
    fn new(name: &str, result: ToolResult) -> Self {
        Self {
            name: name.to_string(),
            result,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for MockTool {
    fn definition(&self) -> CoordinatorToolDefinition {
        CoordinatorToolDefinition {
            name: self.name.clone(),
            description: "A mock tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, ToolCoordinatorError> {
        Ok(self.result.clone())
    }
}

#[tokio::test]
async fn test_new_coordinator() {
    let coord = ToolCoordinator::new();
    assert_eq!(coord.tool_count().await, 0);
}

#[tokio::test]
async fn test_register_handler() {
    let coord = ToolCoordinator::new();
    let handler = Arc::new(MockTool::new("test_tool", ToolResult::success("ok")));

    coord.register_handler(handler).await;

    assert!(coord.has_tool("test_tool").await);
    assert_eq!(coord.tool_count().await, 1);
}

#[tokio::test]
async fn test_unregister() {
    let coord = ToolCoordinator::new();
    let handler = Arc::new(MockTool::new("test_tool", ToolResult::success("ok")));

    coord.register_handler(handler).await;
    assert!(coord.has_tool("test_tool").await);

    let removed = coord.unregister("test_tool").await;
    assert!(removed);
    assert!(!coord.has_tool("test_tool").await);
}

#[tokio::test]
async fn test_get_tool_definitions() {
    let coord = ToolCoordinator::new();

    coord
        .register_handler(Arc::new(MockTool::new("tool1", ToolResult::success("ok"))))
        .await;
    coord
        .register_handler(Arc::new(MockTool::new("tool2", ToolResult::success("ok"))))
        .await;

    let defs = coord.get_tool_definitions().await;
    assert_eq!(defs.len(), 2);

    let names: Vec<String> = defs.iter().map(|d| d.name.clone()).collect();
    assert!(names.contains(&"tool1".to_string()));
    assert!(names.contains(&"tool2".to_string()));
}

#[tokio::test]
async fn test_execute_tool() {
    let coord = ToolCoordinator::new();
    coord
        .register_handler(Arc::new(MockTool::new(
            "echo",
            ToolResult::success("hello"),
        )))
        .await;

    let result = coord.execute_tool("echo", json!({}), None).await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(!result.is_error);
    assert_eq!(result.content, "hello");
}

#[tokio::test]
async fn test_execute_tool_not_found() {
    let coord = ToolCoordinator::new();

    let result = coord.execute_tool("nonexistent", json!({}), None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ToolCoordinatorError::ToolNotFound(name) => assert_eq!(name, "nonexistent"),
        _ => panic!("Expected ToolNotFound error"),
    }
}

#[tokio::test]
async fn test_execute_tool_with_timing() {
    let coord = ToolCoordinator::new();
    coord
        .register_handler(Arc::new(MockTool::new(
            "timed",
            ToolResult::success("result"),
        )))
        .await;

    let exec_result = coord
        .execute_tool_with_timing("tool_123".to_string(), "timed".to_string(), json!({}), None)
        .await;

    assert_eq!(exec_result.id, "tool_123");
    assert_eq!(exec_result.name, "timed");
    assert!(!exec_result.result.is_error);
    assert!(exec_result.elapsed_ms < 1000);
}

#[tokio::test]
async fn test_execute_tools_concurrent() {
    let coord = ToolCoordinator::new();

    coord
        .register_handler(Arc::new(MockTool::new("tool_a", ToolResult::success("a"))))
        .await;
    coord
        .register_handler(Arc::new(MockTool::new("tool_b", ToolResult::success("b"))))
        .await;

    let calls = vec![
        ("id1".to_string(), "tool_a".to_string(), json!({})),
        ("id2".to_string(), "tool_b".to_string(), json!({})),
    ];

    let results = coord.execute_tools_concurrent(calls, None).await;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "id1");
    assert_eq!(results[0].result.content, "a");
    assert_eq!(results[1].id, "id2");
    assert_eq!(results[1].result.content, "b");
}

#[tokio::test]
async fn test_list_tool_names() {
    let coord = ToolCoordinator::new();

    coord
        .register_handler(Arc::new(MockTool::new("alpha", ToolResult::success(""))))
        .await;
    coord
        .register_handler(Arc::new(MockTool::new("beta", ToolResult::success(""))))
        .await;

    let names = coord.list_tool_names().await;

    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha".to_string()));
    assert!(names.contains(&"beta".to_string()));
}

#[tokio::test]
async fn test_clear() {
    let coord = ToolCoordinator::new();

    coord
        .register_handler(Arc::new(MockTool::new("tool1", ToolResult::success(""))))
        .await;
    coord
        .register_handler(Arc::new(MockTool::new("tool2", ToolResult::success(""))))
        .await;

    assert_eq!(coord.tool_count().await, 2);

    coord.clear().await;

    assert_eq!(coord.tool_count().await, 0);
}

#[tokio::test]
async fn test_builder() {
    let coord = ToolCoordinatorBuilder::new()
        .with_handler(Arc::new(MockTool::new("tool1", ToolResult::success("a"))))
        .with_handler(Arc::new(MockTool::new("tool2", ToolResult::success("b"))))
        .build()
        .await;

    assert_eq!(coord.tool_count().await, 2);
    assert!(coord.has_tool("tool1").await);
    assert!(coord.has_tool("tool2").await);
}
