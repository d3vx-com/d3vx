//! Tests for the multi-strategy tool.

use serde_json::json;

use super::strategy::Strategy;
use super::tool::MultiStrategyTool;
use crate::tools::types::{Tool, ToolContext};

fn default_context() -> ToolContext {
    ToolContext::default()
}

#[tokio::test]
async fn test_missing_task_returns_error() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool.execute(json!({"strategies": ["concise"]}), &ctx).await;

    assert!(result.is_error, "expected error when task is missing");
    assert!(
        result.content.contains("task"),
        "error message should mention 'task'"
    );
}

#[tokio::test]
async fn test_empty_task_returns_error() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool.execute(json!({"task": "   "}), &ctx).await;

    assert!(
        result.is_error,
        "expected error when task is empty/whitespace"
    );
}

#[tokio::test]
async fn test_default_strategies_generated() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(json!({"task": "Implement a linked list"}), &ctx)
        .await;

    assert!(
        !result.is_error,
        "expected success, got: {}",
        result.content
    );

    let strategies = result.metadata["strategies"]
        .as_array()
        .expect("strategies should be an array");
    // Default max_agents is 2, so we get 2 of the 3 strategies
    assert_eq!(strategies.len(), 2);
}

#[tokio::test]
async fn test_custom_strategies_parsed() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(
            json!({
                "task": "Build a cache",
                "strategies": ["creative", "concise"],
                "max_agents": 3
            }),
            &ctx,
        )
        .await;

    assert!(
        !result.is_error,
        "expected success, got: {}",
        result.content
    );

    let strategies = result.metadata["strategies"]
        .as_array()
        .expect("strategies should be an array");
    assert_eq!(strategies.len(), 2);

    let names: Vec<&str> = strategies
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["creative", "concise"]);
}

#[tokio::test]
async fn test_strategy_prompts_differ() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(
            json!({
                "task": "Sort an array",
                "strategies": ["concise", "thorough", "creative"],
                "max_agents": 3
            }),
            &ctx,
        )
        .await;

    assert!(!result.is_error);

    let strategies = result.metadata["strategies"].as_array().unwrap();
    assert_eq!(strategies.len(), 3);

    let prompts: Vec<String> = strategies
        .iter()
        .map(|s| s["prompt"].as_str().unwrap().to_string())
        .collect();

    // All three prompts should be distinct
    assert_ne!(prompts[0], prompts[1], "concise and thorough should differ");
    assert_ne!(
        prompts[1], prompts[2],
        "thorough and creative should differ"
    );
    assert_ne!(prompts[0], prompts[2], "concise and creative should differ");

    // Each prompt should contain the original task
    for prompt in &prompts {
        assert!(
            prompt.contains("Sort an array"),
            "prompt should contain the original task text"
        );
    }
}

#[tokio::test]
async fn test_evaluation_criteria_included() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(
            json!({
                "task": "Parse CSV files",
                "evaluation_criteria": "speed and memory efficiency"
            }),
            &ctx,
        )
        .await;

    assert!(!result.is_error);

    let criteria = result.metadata["evaluation_criteria"].as_str().unwrap();
    assert_eq!(criteria, "speed and memory efficiency");
}

#[tokio::test]
async fn test_default_evaluation_criteria() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool.execute(json!({"task": "Do something"}), &ctx).await;

    assert!(!result.is_error);

    let criteria = result.metadata["evaluation_criteria"].as_str().unwrap();
    assert!(
        criteria.contains("correctness"),
        "default criteria should mention correctness"
    );
}

#[tokio::test]
async fn test_max_agents_clamped() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    // max_agents=1 should be clamped to 2
    let result = tool
        .execute(
            json!({
                "task": "Write a function",
                "strategies": ["concise", "thorough", "creative"],
                "max_agents": 1
            }),
            &ctx,
        )
        .await;

    assert!(!result.is_error);
    let strategies = result.metadata["strategies"].as_array().unwrap();
    assert_eq!(strategies.len(), 2, "max_agents should be clamped to 2");

    // max_agents=99 should be clamped to 3
    let result2 = tool
        .execute(
            json!({
                "task": "Write a function",
                "strategies": ["concise", "thorough", "creative"],
                "max_agents": 99
            }),
            &ctx,
        )
        .await;

    assert!(!result2.is_error);
    let strategies2 = result2.metadata["strategies"].as_array().unwrap();
    assert_eq!(strategies2.len(), 3, "max_agents should be clamped to 3");
}

#[tokio::test]
async fn test_invalid_strategy_names_fall_back_to_defaults() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(
            json!({
                "task": "Build something",
                "strategies": ["unknown", "invalid"]
            }),
            &ctx,
        )
        .await;

    assert!(!result.is_error);
    let strategies = result.metadata["strategies"].as_array().unwrap();
    // Falls back to all defaults (3), but max_agents=2 so only 2 used
    assert_eq!(strategies.len(), 2);
    assert_eq!(strategies[0]["name"].as_str().unwrap(), "concise");
}

#[tokio::test]
async fn test_recommendation_metadata_present() {
    let tool = MultiStrategyTool::new();
    let ctx = default_context();

    let result = tool
        .execute(json!({"task": "Implement feature X"}), &ctx)
        .await;

    assert!(!result.is_error);
    assert!(
        result.metadata.contains_key("recommendation"),
        "should include recommendation metadata"
    );
    let rec = result.metadata["recommendation"].as_str().unwrap();
    assert!(
        rec.contains("spawn_parallel"),
        "recommendation should mention spawn_parallel"
    );
}

// --- Unit tests for Strategy helpers ---

#[test]
fn test_strategy_from_name_case_insensitive() {
    assert_eq!(Strategy::from_name("Concise"), Some(Strategy::Concise));
    assert_eq!(Strategy::from_name("THOROUGH"), Some(Strategy::Thorough));
    assert_eq!(Strategy::from_name("creative"), Some(Strategy::Creative));
    assert_eq!(Strategy::from_name("unknown"), None);
}

#[test]
fn test_strategy_all_returns_three() {
    assert_eq!(Strategy::all().len(), 3);
}

#[test]
fn test_strategy_prompt_contains_task() {
    let task = "Implement binary search";
    for strat in Strategy::all() {
        let prompt = strat.generate_prompt(task);
        assert!(
            prompt.starts_with(task),
            "{} strategy prompt should start with the task",
            strat.name()
        );
    }
}
