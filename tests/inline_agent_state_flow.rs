//! State Flow Tests for Inline Agents
//!
//! These tests verify that agent events correctly flow to the main chat UI

use d3vx::app::state::{
    AgentLineType, AgentMessageLine, InlineAgentInfo, InlineAgentStatus, InlineAgentUpdate,
};
use std::time::Instant;

/// Test that status updates correctly propagate
#[test]
fn test_status_update_propagates() {
    let mut agent = InlineAgentInfo::new("test-1".to_string(), "Test task".to_string());

    // Simulate status update like the app does
    let update = InlineAgentUpdate::Status(InlineAgentStatus::Completed);
    match update {
        InlineAgentUpdate::Status(status) => agent.status = status,
        _ => panic!("Wrong update type"),
    }

    assert_eq!(agent.status, InlineAgentStatus::Completed);
}

/// Test that tool updates propagate
#[test]
fn test_tool_update_propagates() {
    let mut agent = InlineAgentInfo::new("test-2".to_string(), "Test task".to_string());

    // Simulate tool updates
    InlineAgentUpdate::Tool("Read".to_string()).apply(&mut agent);
    InlineAgentUpdate::Tool("Write".to_string()).apply(&mut agent);

    assert_eq!(agent.tool_count, 2);
    assert!(agent.tools_used.contains(&"Read".to_string()));
    assert!(agent.tools_used.contains(&"Write".to_string()));
}

/// Test that message updates propagate
#[test]
fn test_message_update_propagates() {
    let mut agent = InlineAgentInfo::new("test-3".to_string(), "Test task".to_string());

    let msg = AgentMessageLine {
        content: "Thinking about the problem...".to_string(),
        line_type: AgentLineType::Thinking,
        timestamp: Instant::now(),
    };

    InlineAgentUpdate::Message(msg.clone()).apply(&mut agent);

    assert_eq!(agent.messages.len(), 1);
    assert_eq!(agent.messages[0].content, "Thinking about the problem...");
    assert_eq!(agent.messages[0].line_type, AgentLineType::Thinking);
}

/// Test that action updates propagate
#[test]
fn test_action_update_propagates() {
    let mut agent = InlineAgentInfo::new("test-4".to_string(), "Test task".to_string());

    InlineAgentUpdate::Action("Reading file src/main.rs".to_string()).apply(&mut agent);

    assert_eq!(
        agent.current_action,
        Some("Reading file src/main.rs".to_string())
    );
}

/// Test that output updates propagate
#[test]
fn test_output_update_propagates() {
    let mut agent = InlineAgentInfo::new("test-5".to_string(), "Test task".to_string());

    InlineAgentUpdate::Output("File contents: hello world".to_string()).apply(&mut agent);

    assert!(agent
        .output_lines
        .contains(&"File contents: hello world".to_string()));
}

/// Test complete event flow: Tool → Message → Status → Output
#[test]
fn test_complete_event_flow() {
    let mut agent =
        InlineAgentInfo::new("test-6".to_string(), "Create a markdown file".to_string());

    // 1. Agent starts with action
    InlineAgentUpdate::Action("Creating README.md".to_string()).apply(&mut agent);
    assert!(agent.current_action.is_some());

    // 2. Tool is called
    InlineAgentUpdate::Tool("Write".to_string()).apply(&mut agent);
    assert_eq!(agent.tool_count, 1);

    // 3. Agent sends thinking message
    InlineAgentUpdate::Message(AgentMessageLine {
        content: "Creating the file now...".to_string(),
        line_type: AgentLineType::Thinking,
        timestamp: Instant::now(),
    })
    .apply(&mut agent);
    assert_eq!(agent.messages.len(), 1);

    // 4. Tool output
    InlineAgentUpdate::Output("File created successfully".to_string()).apply(&mut agent);

    // 5. Agent completes
    InlineAgentUpdate::Status(InlineAgentStatus::Completed).apply(&mut agent);
    assert_eq!(agent.status, InlineAgentStatus::Completed);

    // 6. Final message
    InlineAgentUpdate::Message(AgentMessageLine {
        content: "Done! I've created README.md".to_string(),
        line_type: AgentLineType::Text,
        timestamp: Instant::now(),
    })
    .apply(&mut agent);

    // Verify final state
    assert_eq!(agent.status, InlineAgentStatus::Completed);
    assert_eq!(agent.tool_count, 1);
    assert_eq!(agent.messages.len(), 2);
    assert!(agent
        .output_lines
        .contains(&"File created successfully".to_string()));
}

/// Test that Done event sets status correctly (before Finished)
#[test]
fn test_done_before_finished_order() {
    let mut agent = InlineAgentInfo::new("test-7".to_string(), "Test task".to_string());

    // Simulate the event order: Done comes first, then Finished
    // Done should set status to Completed
    InlineAgentUpdate::Status(InlineAgentStatus::Completed).apply(&mut agent);
    assert_eq!(agent.status, InlineAgentStatus::Completed);

    // Finished should NOT overwrite if already Completed
    // This simulates the fix we made in handlers/agent.rs:1146-1154
    if agent.status == InlineAgentStatus::Running {
        InlineAgentUpdate::Status(InlineAgentStatus::Ended).apply(&mut agent);
    }

    // Status should still be Completed, not Ended
    assert_eq!(agent.status, InlineAgentStatus::Completed);
}

/// Test that Ended is set if status is still Running
#[test]
fn test_ended_when_still_running() {
    let mut agent = InlineAgentInfo::new("test-8".to_string(), "Test task".to_string());

    // Status is Running (default)
    assert_eq!(agent.status, InlineAgentStatus::Running);

    // Finished event comes (simulating the fix logic)
    if agent.status == InlineAgentStatus::Running {
        InlineAgentUpdate::Status(InlineAgentStatus::Ended).apply(&mut agent);
    }

    // Status should now be Ended
    assert_eq!(agent.status, InlineAgentStatus::Ended);
}

/// Test that multiple updates in sequence work correctly
#[test]
fn test_multiple_sequential_updates() {
    let mut agent = InlineAgentInfo::new("test-9".to_string(), "Complex task".to_string());

    // Simulate a complex agent interaction
    let updates = vec![
        InlineAgentUpdate::Action("Starting task".to_string()),
        InlineAgentUpdate::Tool("Read".to_string()),
        InlineAgentUpdate::Message(AgentMessageLine {
            content: "Reading config...".to_string(),
            line_type: AgentLineType::Thinking,
            timestamp: Instant::now(),
        }),
        InlineAgentUpdate::Output("Config loaded".to_string()),
        InlineAgentUpdate::Action("Processing data".to_string()),
        InlineAgentUpdate::Tool("Bash".to_string()),
        InlineAgentUpdate::Message(AgentMessageLine {
            content: "Running tests...".to_string(),
            line_type: AgentLineType::ToolCall,
            timestamp: Instant::now(),
        }),
        InlineAgentUpdate::Output("All tests passed".to_string()),
        InlineAgentUpdate::Status(InlineAgentStatus::Completed),
        InlineAgentUpdate::Message(AgentMessageLine {
            content: "Task completed successfully!".to_string(),
            line_type: AgentLineType::Text,
            timestamp: Instant::now(),
        }),
    ];

    for update in updates {
        update.apply(&mut agent);
    }

    // Verify final state
    assert_eq!(agent.status, InlineAgentStatus::Completed);
    assert_eq!(agent.tool_count, 2);
    assert_eq!(agent.tools_used.len(), 2);
    assert_eq!(agent.messages.len(), 3);
    assert_eq!(agent.output_lines.len(), 2);
}
