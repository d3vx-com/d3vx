//! Tests for Inline Agent Prompt Behavior
//!
//! These tests verify that specialist prompts work correctly
//! without testing specific keywords.

use d3vx::agent::specialists::AgentType;

/// Test that all specialist types have non-empty prompts
#[test]
fn test_all_specialists_have_prompts() {
    let types = vec![
        AgentType::Backend,
        AgentType::Frontend,
        AgentType::Testing,
        AgentType::Documentation,
        AgentType::DevOps,
        AgentType::Security,
        AgentType::Review,
        AgentType::Data,
        AgentType::Mobile,
    ];

    // All specialists should have prompts (General is intentionally empty)
    for agent_type in types {
        let prompt = agent_type.system_prompt_addition();
        assert!(
            !prompt.is_empty(),
            "Specialist {:?} should have a prompt",
            agent_type
        );
    }

    // General intentionally has no additional prompt
    assert!(AgentType::General.system_prompt_addition().is_empty());
}

/// Test that prompts are different from each other
#[test]
fn test_specialist_prompts_are_unique() {
    let prompts: Vec<&str> = vec![
        AgentType::Backend.system_prompt_addition(),
        AgentType::Frontend.system_prompt_addition(),
        AgentType::Testing.system_prompt_addition(),
        AgentType::Documentation.system_prompt_addition(),
        AgentType::DevOps.system_prompt_addition(),
        AgentType::Security.system_prompt_addition(),
        AgentType::Review.system_prompt_addition(),
        AgentType::Data.system_prompt_addition(),
        AgentType::Mobile.system_prompt_addition(),
    ];

    // Filter out empty strings
    let non_empty: Vec<&str> = prompts.into_iter().filter(|p| !p.is_empty()).collect();

    // Check uniqueness
    let mut seen = std::collections::HashSet::new();
    for prompt in &non_empty {
        assert!(
            seen.insert(prompt),
            "Each specialist prompt should be unique"
        );
    }
}

/// Test that agent types can be created and cloned, and serialized
#[test]
fn test_agent_type_properties() {
    // Test display names
    assert!(!AgentType::Backend.display_name().is_empty());
    assert!(!AgentType::Frontend.display_name().is_empty());
    assert!(!AgentType::Testing.display_name().is_empty());

    // Test icons
    assert!(!AgentType::Backend.icon().is_empty());
    assert!(!AgentType::Frontend.icon().is_empty());

    // Test priorities
    assert!(AgentType::General.priority() < AgentType::Backend.priority());
    assert!(AgentType::Security.priority() > AgentType::Backend.priority());
}
