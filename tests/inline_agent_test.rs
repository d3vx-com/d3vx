//! Inline Agent Tests
//!
//! These tests verify the inline agent functionality without building the full TUI.

use d3vx::agent::specialists::AgentType;
use d3vx::app::state::{InlineAgentInfo, InlineAgentStatus};

#[test]
fn test_inline_agent_creation() {
    let agent = InlineAgentInfo::new("test-1".to_string(), "Create a markdown file".to_string());
    assert_eq!(agent.id, "test-1");
    assert_eq!(agent.task, "Create a markdown file");
    assert_eq!(agent.status, InlineAgentStatus::Running);
    assert!(!agent.expanded);
    assert!(!agent.show_tools);
    assert!(agent.tools_used.is_empty());
    assert_eq!(agent.tool_count, 0);
}

#[test]
fn test_inline_agent_toggle_show_tools() {
    let mut agent = InlineAgentInfo::new("test-2".to_string(), "Test task".to_string());

    assert!(!agent.show_tools, "Should default to false");

    // Toggle show_tools
    agent.show_tools = true;
    assert!(agent.show_tools);

    // Toggle back
    agent.show_tools = false;
    assert!(!agent.show_tools);
}

#[test]
fn test_inline_agent_add_tools() {
    let mut agent = InlineAgentInfo::new("test-3".to_string(), "Test task".to_string());

    assert_eq!(agent.tool_count, 0);
    assert!(agent.tools_used.is_empty());

    // Add first tool
    agent.add_tool("Read".to_string());
    assert_eq!(agent.tool_count, 1);
    assert!(agent.tools_used.contains(&"Read".to_string()));

    // Add second tool
    agent.add_tool("Write".to_string());
    assert_eq!(agent.tool_count, 2);
    assert_eq!(agent.tools_used.len(), 2);

    // Add duplicate tool - count increments but unique stays same
    agent.add_tool("Read".to_string());
    assert_eq!(agent.tool_count, 3); // Count increments
    assert_eq!(agent.tools_used.len(), 2); // But unique tools stay at 2
}

#[test]
fn test_inline_agent_status_transitions() {
    let mut agent = InlineAgentInfo::new("test-4".to_string(), "Test task".to_string());
    assert_eq!(agent.status, InlineAgentStatus::Running);

    // Complete the agent
    agent.status = InlineAgentStatus::Completed;
    assert_eq!(agent.status, InlineAgentStatus::Completed);

    // Test progress summary reflects status
    let summary = agent.progress_summary();
    assert!(summary.contains("Done"));
}

#[test]
fn test_inline_agent_progress_summary_running() {
    let agent = InlineAgentInfo::new("test-5".to_string(), "Test task".to_string());
    let summary = agent.progress_summary();
    assert!(summary.contains("Running") || summary.contains("s"));
}

#[test]
fn test_specialist_types_exist() {
    // Verify all specialist types are available
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

    // Each specialist type should have a prompt (General intentionally has none)
    for agent_type in types {
        let prompt = agent_type.system_prompt_addition();
        assert!(
            !prompt.is_empty(),
            "Specialist prompt for {:?} should not be empty",
            agent_type
        );
    }

    // General type intentionally has no additional prompt
    assert!(AgentType::General.system_prompt_addition().is_empty());
}
