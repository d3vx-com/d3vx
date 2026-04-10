//! Tests for SpawnParallel tool

use crate::tools::spawn_parallel::SpawnParallelTool;
use crate::tools::{Tool, ToolContext};
use tokio::sync::mpsc;

/// Helper: spawns a responder task that receives events and replies on the oneshot.
/// Returns a oneshot that will receive the event data (after the responder replies).
fn spawn_responder(
    mut rx: mpsc::Receiver<crate::tools::spawn_parallel::SpawnParallelEvent>,
) -> tokio::sync::oneshot::Receiver<crate::tools::spawn_parallel::SpawnParallelEvent> {
    let (event_tx, event_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if let Some(mut event) = rx.recv().await {
            let n = event.tasks.len();
            // Build a summary that matches what the tool's summary text shows
            let summary = event
                .tasks
                .iter()
                .enumerate()
                .map(|(i, t)| format!("  {}. {}", i + 1, t.description))
                .collect::<Vec<_>>()
                .join("\n");
            let report = format!(
                "Compiled Parallel Execution Report (batch: {}):\nSpawning {n} parallel specialist agents:\n{summary}",
                event.batch_id
            );
            // Take the response_tx out so we don't move the rest of the event
            let response_tx =
                std::mem::replace(&mut event.response_tx, tokio::sync::oneshot::channel().0);
            let _ = response_tx.send(report);
            let _ = event_tx.send(event);
        }
    });
    event_rx
}

#[tokio::test]
async fn test_spawn_parallel_tool_sends_event() {
    let (tx, rx) = mpsc::channel(32);
    let responder = spawn_responder(rx);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {
                "description": "Test task 1",
                "task": "Do thing 1",
                "agent_type": "backend"
            },
            {
                "description": "Test task 2",
                "task": "Do thing 2",
                "agent_type": "Frontend"
            }
        ],
        "reasoning": "These are independent tasks"
    });

    let context = ToolContext::default();
    let result = tool.execute(input, &context).await;

    assert!(
        !result.is_error,
        "Tool should not return error: {}",
        result.content
    );
    assert!(
        result
            .content
            .contains("Spawning 2 parallel specialist agents"),
        "Output: {}",
        result.content
    );
    assert!(
        result.content.contains("Test task 1"),
        "Output: {}",
        result.content
    );

    // Verify event was sent
    let event = responder.await.expect("Event should be sent");
    assert_eq!(event.reasoning, "These are independent tasks");
    assert!(!event.select_best);
    assert!(event.selection_criteria.is_none());
    assert_eq!(event.tasks.len(), 2);
    assert_eq!(event.tasks[0].key, "child-1");
    assert_eq!(event.tasks[0].description, "Test task 1");
    assert_eq!(event.tasks[0].task, "Do thing 1");
    assert_eq!(event.tasks[1].task, "Do thing 2");
}

#[tokio::test]
async fn test_spawn_parallel_tool_validation() {
    let (tx, _rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);

    // Test missing subtasks
    let input = serde_json::json!({
        "reasoning": "No subtasks"
    });
    let context = ToolContext::default();
    let result = tool.execute(input, &context).await;
    assert!(result.is_error);
    assert!(result.content.contains("Missing 'subtasks'"));

    // Test less than 2 subtasks
    let input = serde_json::json!({
        "subtasks": [{"description": "Only one", "task": "Do it"}],
        "reasoning": "Only one task"
    });
    let result = tool.execute(input, &context).await;
    assert!(result.is_error);
    assert!(result.content.contains("Need at least 2"));

    // Test more than 5 subtasks
    let input = serde_json::json!({
        "subtasks": [
            {"description": "1", "task": "t1"},
            {"description": "2", "task": "t2"},
            {"description": "3", "task": "t3"},
            {"description": "4", "task": "t4"},
            {"description": "5", "task": "t5"},
            {"description": "6", "task": "t6"},
        ],
        "reasoning": "Too many"
    });
    let result = tool.execute(input, &context).await;
    assert!(result.is_error);
    assert!(result.content.contains("Maximum 5"));
}

#[tokio::test]
async fn test_spawn_parallel_tool_default_agent_type() {
    let (tx, rx) = mpsc::channel(32);
    let responder = spawn_responder(rx);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {"description": "Task without type", "task": "Do something"},
            {"description": "Task with type", "task": "Do other", "agent_type": "testing"}
        ],
        "reasoning": "Testing defaults"
    });

    let context = ToolContext::default();
    tool.execute(input, &context).await;

    let event = responder.await.expect("Event should be sent");
    // Default is General (Agent in display)
    assert_eq!(event.tasks[0].agent_type.display_name(), "Agent");
    assert_eq!(event.tasks[1].agent_type.display_name(), "QA Engineer");
}

#[tokio::test]
async fn test_spawn_parallel_tool_blocks_recursive_delegation() {
    let (tx, mut rx) = mpsc::channel(32);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {"description": "Task one", "task": "Do one"},
            {"description": "Task two", "task": "Do two"}
        ],
        "reasoning": "still independent"
    });

    let context = ToolContext {
        session_id: Some("child-session".to_string()),
        agent_depth: 1,
        allow_parallel_spawn: false,
        ..Default::default()
    };
    let result = tool.execute(input, &context).await;

    assert!(result.is_error);
    assert!(result.content.contains("cannot spawn more agents"));
    assert!(
        rx.try_recv().is_err(),
        "recursive spawn should not emit an event"
    );
}

#[tokio::test]
async fn test_spawn_parallel_tool_parses_dependencies_and_ownership() {
    let (tx, rx) = mpsc::channel(32);
    let responder = spawn_responder(rx);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {"key": "backend", "description": "Backend", "task": "Implement API", "ownership": "src/api"},
            {"key": "tests", "description": "Tests", "task": "Add integration tests", "depends_on": ["backend"], "ownership": "tests/api"}
        ],
        "reasoning": "Tests should wait for backend"
    });

    let context = ToolContext::default();
    let result = tool.execute(input, &context).await;

    assert!(!result.is_error);
    let event = responder.await.expect("Event should be sent");
    assert_eq!(event.tasks[0].key, "backend");
    assert_eq!(event.tasks[0].ownership.as_deref(), Some("src/api"));
    assert_eq!(event.tasks[1].depends_on, vec!["backend".to_string()]);
    assert_eq!(event.tasks[1].ownership.as_deref(), Some("tests/api"));
}

#[tokio::test]
async fn test_spawn_parallel_tool_parses_best_of_n_batch_options() {
    let (tx, rx) = mpsc::channel(32);
    let responder = spawn_responder(rx);
    let tool = SpawnParallelTool::with_sender(tx);

    let input = serde_json::json!({
        "subtasks": [
            {"key": "candidate-a", "description": "Candidate A", "task": "Implement approach A"},
            {"key": "candidate-b", "description": "Candidate B", "task": "Implement approach B"}
        ],
        "reasoning": "Compare two implementations",
        "select_best": true,
        "selection_criteria": "Prefer the safer and simpler implementation with better testability"
    });

    let context = ToolContext::default();
    let result = tool.execute(input, &context).await;

    assert!(!result.is_error);

    let event = responder.await.expect("Event should be sent");
    assert!(event.select_best);
    assert_eq!(
        event.selection_criteria.as_deref(),
        Some("Prefer the safer and simpler implementation with better testability")
    );
}
