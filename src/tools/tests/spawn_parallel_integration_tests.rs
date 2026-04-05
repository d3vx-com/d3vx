//! Integration test for SpawnParallel tool and event flow

use crate::agent::specialists::AgentType;
use crate::tools::spawn_parallel::{SpawnParallelEvent, SpawnParallelTool, SpawnTask};
use crate::tools::{Tool, ToolContext};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_spawn_parallel_tool_sends_event() {
    let (tx, mut rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {
                "description": "Backend API",
                "task": "Implement REST endpoints",
                "agent_type": "backend"
            },
            {
                "description": "Frontend UI",
                "task": "Build React components",
                "agent_type": "frontend"
            },
            {
                "description": "Tests",
                "task": "Write integration tests",
                "agent_type": "testing"
            }
        ],
        "reasoning": "These are independent tasks"
    });

    let context = ToolContext::default();
    let result = tool.execute(input, &context).await;

    // Tool should succeed
    assert!(
        !result.is_error,
        "Tool should not return error: {}",
        result.content
    );
    assert!(result
        .content
        .contains("Spawning 3 parallel specialist agents"));

    // Event should be sent
    let event = rx.try_recv().expect("Event should be sent");
    assert_eq!(event.tasks.len(), 3);
    assert_eq!(event.tasks[0].description, "Backend API");
    assert_eq!(event.tasks[0].agent_type, AgentType::Backend);
    assert_eq!(event.tasks[1].agent_type, AgentType::Frontend);
    assert_eq!(event.tasks[2].agent_type, AgentType::Testing);
}

#[tokio::test]
async fn test_spawn_parallel_event_forwarding() {
    // Simulate the event forwarding loop
    let (tx, mut rx) = mpsc::channel(32);

    // Spawn the forwarder task (like in App::new)
    let (forward_tx, mut forward_rx) = mpsc::channel(32);
    tokio::spawn(async move {
        while let Some(spawn_event) = rx.recv().await {
            let _ = forward_tx.send(spawn_event).await;
        }
    });

    // Create and execute tool
    let tool = SpawnParallelTool::with_sender(tx);
    let input = serde_json::json!({
        "subtasks": [
            {"description": "Task 1", "task": "Do thing 1", "agent_type": "backend"},
            {"description": "Task 2", "task": "Do thing 2", "agent_type": "frontend"}
        ],
        "reasoning": "Test reasoning"
    });

    let context = ToolContext::default();
    tool.execute(input, &context).await;

    // Event should be forwarded
    let event = forward_rx.recv().await.expect("Event should be forwarded");
    assert_eq!(event.tasks.len(), 2);
}

#[tokio::test]
async fn test_spawn_parallel_tool_validation_errors() {
    let (tx, _rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);
    let context = ToolContext::default();

    // Missing subtasks
    let result = tool
        .execute(serde_json::json!({"reasoning": "test"}), &context)
        .await;
    assert!(result.is_error);
    assert!(result.content.contains("Missing 'subtasks'"));

    // Only 1 subtask
    let (tx, _rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);
    let result = tool
        .execute(
            serde_json::json!({
                "subtasks": [{"description": "Only one", "task": "Do it"}],
                "reasoning": "Only one task"
            }),
            &context,
        )
        .await;
    assert!(result.is_error);
    assert!(result.content.contains("at least 2"));

    // More than 5 subtasks
    let (tx, _rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);
    let too_many = serde_json::json!({
        "subtasks": [
            {"description": "Task 1", "task": "Do thing 1"},
            {"description": "Task 2", "task": "Do thing 2"},
            {"description": "Task 3", "task": "Do thing 3"},
            {"description": "Task 4", "task": "Do thing 4"},
            {"description": "Task 5", "task": "Do thing 5"},
            {"description": "Task 6", "task": "Do thing 6"}
        ],
        "reasoning": "Too many"
    });
    let result = tool.execute(too_many, &context).await;
    assert!(result.is_error);
    assert!(result.content.contains("Maximum 5"));
}

#[tokio::test]
async fn test_spawn_parallel_default_agent_type() {
    let (tx, mut rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);

    // Without agent_type, should default to General
    let input = serde_json::json!({
        "subtasks": [
            {"description": "Task without type", "task": "Do something"},
            {"description": "Task with type", "task": "Do other", "agent_type": "testing"}
        ],
        "reasoning": "Testing defaults"
    });

    let context = ToolContext::default();
    tool.execute(input, &context).await;

    let event = rx.try_recv().expect("Event should be sent");
    assert_eq!(event.tasks[0].agent_type, AgentType::General);
    assert_eq!(event.tasks[1].agent_type, AgentType::Testing);
}

#[tokio::test]
async fn test_spawn_task_struct() {
    let task = SpawnTask {
        key: "backend".to_string(),
        description: "Test task".to_string(),
        task: "Do something".to_string(),
        agent_type: AgentType::Backend,
        depends_on: Vec::new(),
        ownership: Some("src/api".to_string()),
        model: None,
        max_turns: None,
    };

    let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
    let event = SpawnParallelEvent::new(
        "batch-1".to_string(),
        Some("parent-session".to_string()),
        "test batch".to_string(),
        false,
        None,
        vec![task],
        response_tx,
    );
    assert_eq!(event.tasks.len(), 1);
    assert_eq!(event.batch_id, "batch-1");
    assert_eq!(event.tasks[0].description, "Test task");
    assert_eq!(event.tasks[0].ownership.as_deref(), Some("src/api"));
}

#[tokio::test]
async fn test_spawn_parallel_tool_without_sender() {
    // Tool without sender should still return success (graceful degradation)
    let tool = SpawnParallelTool::new();
    let context = ToolContext::default();

    let input = serde_json::json!({
        "subtasks": [
            {"description": "Task 1", "task": "Do thing 1"},
            {"description": "Task 2", "task": "Do thing 2"}
        ],
        "reasoning": "Test"
    });

    let result = tool.execute(input, &context).await;
    // Should succeed even without sender (event not sent)
    assert!(!result.is_error);
    assert!(result.content.contains("Spawning 2"));
}
