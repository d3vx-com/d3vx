//! Pipeline prompts tests

use super::super::phases::Phase;
use super::instructions::build_phase_instruction;
use super::system_prompts::get_system_prompt;

#[test]
fn test_get_system_prompt_research() {
    let prompt = get_system_prompt(Phase::Research);
    assert!(prompt.contains("RESEARCHER"));
    assert!(prompt.contains("NOT modify"));
}

#[test]
fn test_get_system_prompt_plan() {
    let prompt = get_system_prompt(Phase::Plan);
    assert!(prompt.contains("PLANNER"));
    assert!(prompt.contains("JSON"));
}

#[test]
fn test_get_system_prompt_implement() {
    let prompt = get_system_prompt(Phase::Implement);
    assert!(prompt.contains("IMPLEMENTER"));
    assert!(prompt.contains("production-ready"));
}

#[test]
fn test_get_system_prompt_review() {
    let prompt = get_system_prompt(Phase::Review);
    assert!(prompt.contains("REVIEWER"));
    assert!(prompt.contains("APPROVED"));
    assert!(prompt.contains("FIXED"));
}

#[test]
fn test_get_system_prompt_docs() {
    let prompt = get_system_prompt(Phase::Docs);
    assert!(prompt.contains("DOCUMENTATION"));
    assert!(prompt.contains("CHANGELOG"));
}

#[test]
fn test_build_phase_instruction_research() {
    let instruction = build_phase_instruction(
        Phase::Research,
        "Add authentication",
        "Implement JWT-based authentication",
        "TASK-001",
        Some("Previous session context"),
        Some("Use TypeScript strict mode"),
        None,
    );

    assert!(instruction.contains("Research Phase"));
    assert!(instruction.contains("Add authentication"));
    assert!(instruction.contains("JWT-based authentication"));
    assert!(instruction.contains("Previous session context"));
    assert!(instruction.contains("TypeScript strict mode"));
    assert!(instruction.contains(".d3vx/research-TASK-001.md"));
}

#[test]
fn test_build_phase_instruction_plan() {
    let instruction = build_phase_instruction(
        Phase::Plan,
        "Add feature X",
        "Description",
        "TASK-002",
        None,
        None,
        Some("Ignore deprecated files"),
    );

    assert!(instruction.contains("Plan Phase"));
    assert!(instruction.contains(".d3vx/plan-TASK-002.json"));
    assert!(instruction.contains("Ignore deprecated files"));
}

#[test]
fn test_build_phase_instruction_implement() {
    let instruction = build_phase_instruction(
        Phase::Implement,
        "Task",
        "Description",
        "TASK-003",
        None,
        None,
        None,
    );

    assert!(instruction.contains("Implement Phase"));
    assert!(instruction.contains("cargo check"));
    assert!(instruction.contains("git commit"));
}

#[test]
fn test_all_phases_have_prompts() {
    for phase in Phase::all() {
        let prompt = get_system_prompt(*phase);
        assert!(!prompt.is_empty(), "Missing prompt for {:?}", phase);
    }
}
