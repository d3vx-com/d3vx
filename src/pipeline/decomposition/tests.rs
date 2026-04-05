//! Tests for task decomposition

use super::aggregator::ResultAggregator;
use super::decomposer::TaskDecomposer;
use super::dependency_graph::DependencyGraph;
use super::types::{
    AggregationStrategy, ChildTaskDefinition, ChildTaskStatus, DecompositionPlan,
    DecompositionStatus,
};
use crate::pipeline::phases::{Task, TaskStatus};

#[test]
fn test_decomposition_id() {
    let id = super::types::DecompositionId(42);
    assert_eq!(id.to_string(), "decomp-42");
}

#[test]
fn test_decomposition_plan_creation() {
    let plan = DecompositionPlan::new("TASK-001");
    assert_eq!(plan.parent_task_id, "TASK-001");
    assert_eq!(plan.status, DecompositionStatus::Planned);
    assert!(plan.children.is_empty());
}

#[test]
fn test_add_child_to_plan() {
    let mut plan = DecompositionPlan::new("TASK-001");

    let child = ChildTaskDefinition {
        key: "backend".to_string(),
        title: "Backend API".to_string(),
        instruction: "Implement the API".to_string(),
        complexity: 0.5,
        depends_on: vec![],
        estimated_duration: Some(30),
        tags: vec!["backend".to_string()],
        priority: None,
        initial_phase: None,
    };

    plan.add_child(child);
    assert_eq!(plan.children.len(), 1);
}

#[test]
fn test_dependency_graph() {
    let mut graph = DependencyGraph::new();

    // Add nodes with dependencies
    graph.add_dependency("frontend", "backend");
    graph.add_dependency("backend", "database");
    graph.add_node(&"database".to_string());

    let roots = graph.get_roots();
    assert_eq!(roots.len(), 1);
    assert!(roots.contains(&"database".to_string()));

    let levels = graph.get_execution_levels();
    assert_eq!(levels.len(), 3); // database -> backend -> frontend
}

#[test]
fn test_dependency_graph_no_cycle() {
    let mut graph = DependencyGraph::new();

    graph.add_node(&"a".to_string());
    graph.add_node(&"b".to_string());
    graph.add_node(&"c".to_string());

    // Linear: a -> b -> c
    graph.add_dependency("b", "a");
    graph.add_dependency("c", "b");

    assert!(graph.validate().is_ok());
}

#[test]
fn test_execution_levels() {
    let mut graph = DependencyGraph::new();

    // Create diamond: root -> [left, right] -> merge
    graph.add_dependency("left", "root");
    graph.add_dependency("right", "root");
    graph.add_dependency("merge", "left");
    graph.add_dependency("merge", "right");

    let levels = graph.get_execution_levels();
    assert_eq!(levels.len(), 3);
    assert!(levels[0].contains(&"root".to_string()));
    assert!(levels[1].contains(&"left".to_string()));
    assert!(levels[1].contains(&"right".to_string()));
    assert!(levels[2].contains(&"merge".to_string()));
}

#[test]
fn test_result_aggregator_all_success() {
    let aggregator = ResultAggregator::new(AggregationStrategy::AllSuccess);

    let statuses = vec![
        ChildTaskStatus {
            key: "a".to_string(),
            task_id: Some("CHILD-001".to_string()),
            status: TaskStatus::Completed,
            result: Some("Done".to_string()),
            error: None,
            started_at: None,
            completed_at: None,
        },
        ChildTaskStatus {
            key: "b".to_string(),
            task_id: Some("CHILD-002".to_string()),
            status: TaskStatus::Completed,
            result: Some("Done".to_string()),
            error: None,
            started_at: None,
            completed_at: None,
        },
    ];

    let (status, _) = aggregator.aggregate(&statuses);
    assert_eq!(status, DecompositionStatus::Completed);
}

#[test]
fn test_result_aggregator_partial() {
    let aggregator = ResultAggregator::new(AggregationStrategy::AllSuccess);

    let statuses = vec![
        ChildTaskStatus {
            key: "a".to_string(),
            task_id: Some("CHILD-001".to_string()),
            status: TaskStatus::Completed,
            result: Some("Done".to_string()),
            error: None,
            started_at: None,
            completed_at: None,
        },
        ChildTaskStatus {
            key: "b".to_string(),
            task_id: Some("CHILD-002".to_string()),
            status: TaskStatus::Failed,
            result: None,
            error: Some("Task failed".to_string()),
            started_at: None,
            completed_at: None,
        },
    ];

    let (status, _) = aggregator.aggregate(&statuses);
    assert_eq!(status, DecompositionStatus::Failed);
}

#[test]
fn test_task_decomposer() {
    let decomposer = TaskDecomposer::new();

    let task = Task::new("TASK-001", "Test", "Test task");
    assert!(!decomposer.should_decompose(&task));
}

#[test]
fn test_decomposer_with_high_complexity() {
    let decomposer = TaskDecomposer::new();

    let mut task = Task::new("TASK-001", "Complex task", "Complex task");
    task.metadata = serde_json::json!({
        "complexity_score": 0.9,
        "should_decompose": true
    });

    assert!(decomposer.should_decompose(&task));
}
