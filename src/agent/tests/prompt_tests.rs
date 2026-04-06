//! Tests for prompt module — Role enum, build_system_prompt

use crate::agent::prompt::{build_system_prompt, build_system_prompt_with_options, Role};

// ─── Role enum tests ───────────────────────────────────────────────────────

#[test]
fn test_role_variants_exist() {
    let roles = vec![
        Role::TechLead,
        Role::Executor,
        Role::BackendDev,
        Role::FrontendDev,
        Role::DevOps,
        Role::QA,
    ];
    assert_eq!(roles.len(), 6);
}

#[test]
fn test_role_descriptions_non_empty() {
    for role in [
        Role::TechLead,
        Role::Executor,
        Role::BackendDev,
        Role::FrontendDev,
        Role::DevOps,
        Role::QA,
    ] {
        let desc = role.description();
        assert!(!desc.is_empty(), "Role {:?} has empty description", role);
    }
}

#[test]
fn test_role_techlead_description_mentions_parallels() {
    let desc = Role::TechLead.description();
    assert!(
        desc.contains("SpawnParallel") || desc.contains("spawn_parallel"),
        "TechLead role should mention parallel spawning"
    );
}

#[test]
fn test_role_executor_description_mentions_boundaries() {
    let desc = Role::Executor.description();
    assert!(
        desc.to_lowercase().contains("boundary") || desc.to_lowercase().contains("scope"),
        "Executor role should mention working within boundaries"
    );
}

#[test]
fn test_role_backend_mentions_api() {
    let desc = Role::BackendDev.description();
    let lower = desc.to_lowercase();
    assert!(
        lower.contains("api") || lower.contains("database"),
        "Backend role should mention API/database"
    );
}

#[test]
fn test_role_qa_mentions_testing() {
    let desc = Role::QA.description();
    let lower = desc.to_lowercase();
    assert!(
        lower.contains("test") || lower.contains("qa"),
        "QA role should mention testing"
    );
}

#[test]
fn test_role_serialization_roundtrip() {
    for role in [
        Role::TechLead,
        Role::Executor,
        Role::BackendDev,
        Role::FrontendDev,
        Role::DevOps,
        Role::QA,
    ] {
        let json = serde_json::to_string(&role).unwrap();
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(role, parsed);
    }
}

// ─── build_system_prompt tests ──────────────────────────────────────────────

#[test]
fn test_build_system_prompt_contains_core_identity() {
    let prompt = build_system_prompt("/tmp", None);
    assert!(
        prompt.contains("d3vx"),
        "Prompt should contain agent identity"
    );
}

#[test]
fn test_build_system_prompt_contains_environment() {
    let prompt = build_system_prompt("/tmp", None);
    assert!(prompt.contains("Current Working Directory"));
    assert!(prompt.contains("/tmp"));
}

#[test]
fn test_build_system_prompt_with_role() {
    let prompt = build_system_prompt("/tmp", Some(&Role::QA));
    assert!(prompt.contains("QA"));
}

#[test]
fn test_build_system_prompt_with_techlead_role() {
    let prompt = build_system_prompt("/tmp", Some(&Role::TechLead));
    assert!(prompt.contains("spawn_parallel") || prompt.contains("SpawnParallel"));
}

// ─── Parallel agents prompt tests ──────────────────────────────────────────

#[test]
fn test_codebase_analysis_with_parallel_agents() {
    let prompt = build_system_prompt_with_options("/tmp", None, true);

    println!("=== Full System Prompt for Codebase Analysis ===");
    println!("{}", prompt);
    println!("\n=== End of Prompt ===\n");

    // Verify the prompt contains necessary elements for codebase analysis
    assert!(
        prompt.contains("spawn_parallel_agents"),
        "Prompt should mention spawn_parallel_agents tool"
    );
    assert!(
        prompt.contains("PARALLEL") || prompt.contains("parallel"),
        "Prompt should mention parallelism"
    );
    assert!(prompt.contains("agent"), "Prompt should mention agents");

    // Check that the prompt guides toward parallel execution
    assert!(
        prompt.contains("coordinator") || prompt.contains("orchestrat"),
        "Prompt should identify agent as coordinator/orchestrator"
    );
}

#[test]
fn test_analysis_workflow_in_prompt() {
    let prompt = build_system_prompt_with_options("/tmp", None, true);

    // For "analyze codebase using parallel agents" workflow:
    // The prompt should guide the LLM to:
    // 1. Explore structure first
    // 2. Break into parts
    // 3. Spawn parallel agents

    // Check that the prompt encourages breaking tasks into parts
    assert!(
        prompt.contains("break") || prompt.contains("split") || prompt.contains("independent"),
        "Prompt should encourage breaking tasks into parts"
    );

    // Check that agent types are available for different analysis needs
    let analysis_types = vec!["backend", "frontend", "review", "general"];
    for atype in analysis_types {
        assert!(
            prompt.to_lowercase().contains(atype),
            "Prompt should mention agent type: {}",
            atype
        );
    }
}

#[test]
fn test_spawn_parallel_agents_tool_call_format() {
    let prompt = build_system_prompt_with_options("/tmp", None, true);

    // Verify the prompt shows how to call spawn_parallel_agents
    // The format should be clear enough for the LLM to understand

    // Should mention the tool name
    assert!(
        prompt.contains("spawn_parallel_agents"),
        "Tool name should be in prompt"
    );

    // Should mention subtasks structure
    assert!(
        prompt.contains("subtasks"),
        "Should mention subtasks structure"
    );

    // Should mention agent_type
    assert!(prompt.contains("agent_type"), "Should mention agent_type");

    // Should mention reasoning
    assert!(
        prompt.contains("reasoning"),
        "Should mention reasoning field"
    );
}

#[test]
fn test_analysis_use_case_examples() {
    let prompt = build_system_prompt_with_options("/tmp", None, true);
    let prompt_lower = prompt.to_lowercase();

    // For codebase analysis, the prompt should support breaking by:
    // - modules/packages
    // - directories
    // - layers (frontend/backend/data)

    // Check that the prompt doesn't restrict to specific examples
    // and is flexible enough for analysis tasks

    // The prompt should encourage using parallel agents for multi-part tasks
    assert!(
        prompt_lower.contains("parallel") || prompt_lower.contains("simultaneous"),
        "Prompt should encourage parallel execution"
    );
}
