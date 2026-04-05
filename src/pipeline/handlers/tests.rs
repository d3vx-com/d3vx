//! Tests for phase handlers

use super::docs::DocsHandler;
use super::factory::{create_handler, default_handlers};
use super::implement::ImplementHandler;
use super::plan::PlanHandler;
use super::research::ResearchHandler;
use super::review::ReviewHandler;
use super::types::{PhaseHandler, PhaseResult};
use crate::pipeline::phases::{Phase, PhaseContext, Task};

fn create_test_task() -> Task {
    Task::new("TEST-001", "Test task", "Test instruction")
}

fn create_test_context(task: Task) -> PhaseContext {
    PhaseContext::new(task, "/project/root", "/project/worktree")
}

#[test]
fn test_phase_result_success() {
    let result = PhaseResult::success("All good");
    assert!(result.success);
    assert_eq!(result.output, "All good");
    assert!(!result.has_errors());
}

#[test]
fn test_phase_result_failure() {
    let result = PhaseResult::failure("Something went wrong");
    assert!(!result.success);
    assert!(result.has_errors());
    assert!(result.errors.contains(&"Something went wrong".to_string()));
}

#[test]
fn test_phase_result_builder() {
    let result = PhaseResult::success("Done")
        .with_modified_file("src/main.rs")
        .with_created_file("src/new.rs")
        .with_commit("abc123");

    assert_eq!(result.files_modified.len(), 1);
    assert_eq!(result.files_created.len(), 1);
    assert_eq!(result.commit_hash, Some("abc123".to_string()));
}

#[tokio::test]
async fn test_research_handler_dry_run() {
    let handler = ResearchHandler::new();
    assert_eq!(handler.phase(), Phase::Research);

    let task = create_test_task().with_phase(Phase::Research);
    let context = create_test_context(task.clone());

    // Test without agent (dry-run mode)
    let result = handler.execute(&task, &context, None).await.unwrap();
    assert!(result.success);
    assert!(result.output.contains("dry-run"));
}

#[tokio::test]
async fn test_plan_handler_dry_run() {
    let handler = PlanHandler::new();
    assert_eq!(handler.phase(), Phase::Plan);

    let task = create_test_task().with_phase(Phase::Plan);
    let context = create_test_context(task.clone());

    let result = handler.execute(&task, &context, None).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_implement_handler_dry_run() {
    let handler = ImplementHandler::new();
    assert_eq!(handler.phase(), Phase::Implement);

    let task = create_test_task().with_phase(Phase::Implement);
    let context = create_test_context(task.clone());

    let result = handler.execute(&task, &context, None).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_review_handler_dry_run() {
    let handler = ReviewHandler::new();
    assert_eq!(handler.phase(), Phase::Review);

    let task = create_test_task().with_phase(Phase::Review);
    let context = create_test_context(task.clone());

    let result = handler.execute(&task, &context, None).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_docs_handler_dry_run() {
    let handler = DocsHandler::new();
    assert_eq!(handler.phase(), Phase::Docs);

    let task = create_test_task().with_phase(Phase::Docs);
    let context = create_test_context(task.clone());

    let result = handler.execute(&task, &context, None).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_handler_wrong_phase() {
    let handler = ResearchHandler::new();
    let task = create_test_task().with_phase(Phase::Implement); // Wrong phase
    let context = create_test_context(task.clone());

    let result = handler.execute(&task, &context, None).await;
    assert!(result.is_err());
}

#[test]
fn test_create_handler() {
    let handler = create_handler(Phase::Research);
    assert_eq!(handler.phase(), Phase::Research);

    let handler = create_handler(Phase::Docs);
    assert_eq!(handler.phase(), Phase::Docs);
}

#[test]
fn test_default_handlers() {
    let handlers = default_handlers();
    assert_eq!(handlers.len(), 6);
}

#[test]
fn test_research_instruction_generation() {
    let handler = ResearchHandler::new();
    let task = create_test_task().with_phase(Phase::Research);
    let context = create_test_context(task)
        .with_agent_rules("Use strict mode")
        .with_memory_context("Previous context");

    let instruction = handler.generate_instruction(&context);

    assert!(instruction.contains("Research Phase"));
    assert!(instruction.contains("Test task"));
    assert!(instruction.contains("Previous context"));
    assert!(instruction.contains("Use strict mode"));
    assert!(instruction.contains(".d3vx/research-TEST-001.md"));
}

#[test]
fn test_plan_instruction_generation() {
    let handler = PlanHandler::new();
    let task = create_test_task().with_phase(Phase::Plan);
    let context = create_test_context(task);

    let instruction = handler.generate_instruction(&context);

    assert!(instruction.contains("Plan Phase"));
    assert!(instruction.contains(".d3vx/plan-TEST-001.json"));
}

#[test]
fn test_implement_instruction_generation() {
    let handler = ImplementHandler::new();
    let task = create_test_task().with_phase(Phase::Implement);
    let context = create_test_context(task);

    let instruction = handler.generate_instruction(&context);

    assert!(instruction.contains("Implement Phase"));
    assert!(instruction.contains("cargo check"));
}

#[test]
fn test_review_instruction_generation() {
    let handler = ReviewHandler::new();
    let task = create_test_task().with_phase(Phase::Review);
    let context = create_test_context(task);

    let instruction = handler.generate_instruction(&context);

    assert!(instruction.contains("Review Phase"));
    assert!(instruction.contains("REVIEW: APPROVED"));
}

#[test]
fn test_docs_instruction_generation() {
    let handler = DocsHandler::new();
    let task = create_test_task().with_phase(Phase::Docs);
    let context = create_test_context(task);

    let instruction = handler.generate_instruction(&context);

    assert!(instruction.contains("Docs Phase"));
    assert!(instruction.contains("CHANGELOG"));
}
