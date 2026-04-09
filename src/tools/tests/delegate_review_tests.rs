//! Delegate Review Tool Tests

use crate::tools::delegate_review::DelegateReviewTool;
use crate::tools::types::{Tool, ToolContext};
use serde_json::json;

#[tokio::test]
async fn valid_task_description_returns_success() {
    let tool = DelegateReviewTool::new();
    let ctx = ToolContext::default();

    let r = tool
        .execute(json!({"task_description": "implement login flow"}), &ctx)
        .await;

    assert!(!r.is_error);
    assert!(r.content.contains("Reviewer sub-agent requested"));
    assert!(r.content.contains("implement login flow"));
    assert!(r.content.contains("Senior Quality Assurance Engineer"));
}

#[tokio::test]
async fn missing_task_description_uses_default() {
    let tool = DelegateReviewTool::new();
    let ctx = ToolContext::default();

    let r = tool.execute(json!({}), &ctx).await;

    assert!(!r.is_error);
    assert!(r.content.contains("the current task"));
}

#[test]
fn name_is_delegate_review() {
    let tool = DelegateReviewTool::new();
    let def = tool.definition();
    assert_eq!(def.name, "DelegateReview");
}

#[test]
fn definition_has_required_task_description() {
    let tool = DelegateReviewTool::new();
    let def = tool.definition();
    let req = def.input_schema["required"].as_array().unwrap();
    assert!(req.iter().any(|v| v == "task_description"));
}
