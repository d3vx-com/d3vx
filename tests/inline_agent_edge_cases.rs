//! Edge Case Tests for Inline Agents
//!
//! These tests verify inline agent behavior in edge case scenarios

use d3vx::app::state::{AgentLineType, AgentMessageLine, InlineAgentInfo, InlineAgentStatus};
use std::time::Instant;

/// Test that messages are capped at 100
#[test]
fn test_message_cap_at_100() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    // Add 150 messages
    for i in 0..150 {
        agent.add_message(AgentMessageLine {
            content: format!("Message {}", i),
            line_type: AgentLineType::Text,
            timestamp: Instant::now(),
        });
    }

    // Should be capped at 100
    assert!(
        agent.messages.len() <= 100,
        "Messages should be capped at 100, got {}",
        agent.messages.len()
    );
}

/// Test empty task description
#[test]
fn test_empty_task_description() {
    let agent = InlineAgentInfo::new("test".to_string(), "".to_string());
    assert_eq!(agent.task, "");
    // Should still work with empty task
    let summary = agent.progress_summary();
    assert!(!summary.is_empty());
}

/// Test very long task description
#[test]
fn test_very_long_task_description() {
    let long_task = "x".repeat(10000);
    let agent = InlineAgentInfo::new("test".to_string(), long_task.clone());
    assert_eq!(agent.task.len(), 10000);
}

/// Test tool names with special characters
#[test]
fn test_tool_names_special_characters() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    agent.add_tool("Read-File_v2".to_string());
    agent.add_tool("Bash (sudo)".to_string());
    agent.add_tool("HTTP::Request".to_string());

    assert_eq!(agent.tools_used.len(), 3);
    assert!(agent.tools_used.contains(&"Read-File_v2".to_string()));
    assert!(agent.tools_used.contains(&"Bash (sudo)".to_string()));
    assert!(agent.tools_used.contains(&"HTTP::Request".to_string()));
}

/// Test status can transition multiple times
#[test]
fn test_status_multiple_transitions() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    // Running -> Completed -> Failed (edge case: status changed after completion)
    agent.status = InlineAgentStatus::Completed;
    assert_eq!(agent.status, InlineAgentStatus::Completed);

    // This is an edge case - changing from Completed to Failed
    // (shouldn't happen in practice but we test it works)
    agent.status = InlineAgentStatus::Failed;
    assert_eq!(agent.status, InlineAgentStatus::Failed);
}

/// Test show_tools persists across status changes
#[test]
fn test_show_tools_persists_across_status() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    agent.show_tools = true;
    assert!(agent.show_tools);

    // Change status
    agent.status = InlineAgentStatus::Completed;
    assert!(
        agent.show_tools,
        "show_tools should persist after status change"
    );

    // Change to failed
    agent.status = InlineAgentStatus::Failed;
    assert!(agent.show_tools, "show_tools should persist after failure");
}

/// Test expanded persists across status changes
#[test]
fn test_expanded_persists_across_status() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    agent.expanded = true;
    agent.status = InlineAgentStatus::Completed;

    assert!(
        agent.expanded,
        "expanded should persist after status change"
    );
}

/// Test many rapid tool additions
#[test]
fn test_rapid_tool_additions() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    // Add 1000 tools rapidly
    for i in 0..1000 {
        agent.add_tool(format!("Tool{}", i % 10)); // Only 10 unique tools
    }

    assert_eq!(agent.tool_count, 1000);
    assert_eq!(agent.tools_used.len(), 10, "Should have 10 unique tools");
}

/// Test message order is preserved (FIFO)
#[test]
fn test_message_order_preserved() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    for i in 0..5 {
        agent.add_message(AgentMessageLine {
            content: format!("Msg{}", i),
            line_type: AgentLineType::Text,
            timestamp: Instant::now(),
        });
    }

    // Messages should be in order: [Msg0, Msg1, Msg2, Msg3, Msg4]
    assert_eq!(agent.messages[0].content, "Msg0");
    assert_eq!(agent.messages[1].content, "Msg1");
    assert_eq!(agent.messages[2].content, "Msg2");
    assert_eq!(agent.messages[3].content, "Msg3");
    assert_eq!(agent.messages[4].content, "Msg4");
}

/// Test message capping keeps newest messages
#[test]
fn test_message_capping_keeps_newest() {
    let mut agent = InlineAgentInfo::new("test".to_string(), "Test".to_string());

    // Add messages 0-199 (200 messages, cap is 100)
    for i in 0..200 {
        agent.add_message(AgentMessageLine {
            content: format!("Message {}", i),
            line_type: AgentLineType::Text,
            timestamp: Instant::now(),
        });
    }

    // Should have kept the newer messages (100-199)
    // First message should be around index 100 (after capping)
    let first_msg_num: usize = agent.messages[0]
        .content
        .strip_prefix("Message ")
        .unwrap()
        .parse()
        .unwrap();

    assert!(
        first_msg_num >= 100,
        "Should keep messages 100+, got {}",
        first_msg_num
    );
}
