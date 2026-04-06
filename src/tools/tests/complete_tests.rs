//! Complete Task Tool Tests

use crate::tools::complete::CompleteTaskTool;
use crate::tools::types::{Tool, ToolContext};
use serde_json::json;

#[tokio::test]
async fn valid_summary_returns_success() {
    let tool = CompleteTaskTool::new();
    let ctx = ToolContext::default();

    let r = tool
        .execute(
            json!({"summary": "Added tests and linting passed"}),
            &ctx,
        )
        .await;

    assert!(!r.is_error);
    assert!(r.content.contains("Task marked as complete"));
    assert!(r.content.contains("Added tests"));
}

#[tokio::test]
async fn missing_summary_returns_error() {
    let tool = CompleteTaskTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({}), &ctx).await;

    assert!(r.is_error);
    assert!(r.content.contains("summary"));
}

#[tokio::test]
async fn summary_is_numeric_returns_error() {
    let tool = CompleteTaskTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({"summary": 42}), &ctx).await;

    assert!(r.is_error);
}

#[test]
fn name_is_complete_task() {
    let tool = CompleteTaskTool::new();
    assert_eq!(tool.name(), "complete_task");
}

#[test]
fn definition_has_required_summary() {
    let tool = CompleteTaskTool::new();
    let def = tool.definition();
    assert_eq!(def.name, "complete_task");
    assert!(def.description.contains("complete"));
    let req = def.input_schema["required"].as_array().unwrap();
    assert!(req.iter().any(|v| v == "summary"));
}
