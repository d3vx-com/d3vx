//! Tests for phase handlers

use super::docs::DocsHandler;
use super::factory::{create_handler, default_handlers};
use super::implement::ImplementHandler;
use super::plan::PlanHandler;
use super::research::ResearchHandler;
use super::review::ReviewHandler;
use super::types::{check_agent_safety, PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentResult;
use crate::providers::TokenUsage;
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

// ── check_agent_safety helper ────────────────────────────────────────
//
// Agent safety flags (doom_loop_detected, budget_exhausted) previously
// existed on `AgentResult` but were never inspected by phase handlers —
// runaway-stopped agents were silently treated as successful. These
// tests lock down the conversion from safety flag → PhaseError so a
// future refactor can't regress the wiring.

fn agent_result_ok() -> AgentResult {
    AgentResult {
        text: "ok".to_string(),
        usage: TokenUsage::default(),
        tool_calls: 1,
        iterations: 1,
        task_completed: true,
        budget_exhausted: false,
        doom_loop_detected: false,
    }
}

#[test]
fn check_agent_safety_passes_clean_result_through() {
    let result = agent_result_ok();
    let passed = check_agent_safety(result).expect("clean result must pass through");
    assert!(passed.task_completed);
    assert_eq!(passed.tool_calls, 1);
}

#[test]
fn check_agent_safety_converts_doom_loop_to_error() {
    let mut result = agent_result_ok();
    result.doom_loop_detected = true;
    result.iterations = 4;
    result.tool_calls = 12;

    let err = check_agent_safety(result).unwrap_err();
    match err {
        PhaseError::AgentSafetyStop { reason } => {
            assert!(
                reason.contains("doom loop"),
                "reason must identify doom loop; got: {reason}"
            );
            assert!(
                reason.contains("4") && reason.contains("12"),
                "reason must include iterations/tool_calls for ops context; got: {reason}"
            );
        }
        other => panic!("expected AgentSafetyStop, got {other:?}"),
    }
}

#[test]
fn check_agent_safety_converts_budget_exhausted_to_error() {
    let mut result = agent_result_ok();
    result.budget_exhausted = true;
    result.iterations = 50;
    result.tool_calls = 200;

    let err = check_agent_safety(result).unwrap_err();
    match err {
        PhaseError::AgentSafetyStop { reason } => {
            assert!(
                reason.contains("budget"),
                "reason must identify budget exhaustion; got: {reason}"
            );
        }
        other => panic!("expected AgentSafetyStop, got {other:?}"),
    }
}

#[test]
fn check_agent_safety_prioritises_doom_loop_when_both_flags_set() {
    // If both flags fire in the same run (rare but possible during long
    // sessions) we surface the doom loop — it's the tighter, earlier
    // signal and tells operators the agent was actively looping, not
    // just slowly spending.
    let mut result = agent_result_ok();
    result.doom_loop_detected = true;
    result.budget_exhausted = true;

    let err = check_agent_safety(result).unwrap_err();
    match err {
        PhaseError::AgentSafetyStop { reason } => {
            assert!(reason.contains("doom loop"));
            assert!(!reason.contains("budget"));
        }
        other => panic!("expected AgentSafetyStop, got {other:?}"),
    }
}
