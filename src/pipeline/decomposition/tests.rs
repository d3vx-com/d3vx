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

// ── DependencyGraph::validate cycle detection ──────────────────
//
// Previously `validate` was a no-op: it counted nodes produced by
// `get_execution_levels`, which tolerates cycles by dumping all
// unreachable nodes into a final level. These tests lock in real
// cycle detection so decomposition plans can't silently ship with
// broken dependencies.

#[test]
fn validate_passes_for_empty_graph() {
    let graph = DependencyGraph::new();
    assert!(graph.validate().is_ok());
}

#[test]
fn validate_passes_for_diamond() {
    let mut graph = DependencyGraph::new();
    // root -> [left, right] -> merge
    graph.add_dependency("left", "root");
    graph.add_dependency("right", "root");
    graph.add_dependency("merge", "left");
    graph.add_dependency("merge", "right");
    assert!(graph.validate().is_ok());
}

#[test]
fn validate_detects_two_node_cycle() {
    let mut graph = DependencyGraph::new();
    graph.add_dependency("a", "b");
    graph.add_dependency("b", "a");

    let err = graph.validate().unwrap_err();
    assert!(err.contains("cycle"), "error must identify cycle: {err}");
    assert!(err.contains("a") && err.contains("b"));
}

#[test]
fn validate_detects_self_loop() {
    let mut graph = DependencyGraph::new();
    graph.add_dependency("a", "a");

    let err = graph.validate().unwrap_err();
    assert!(err.contains("cycle"));
    assert!(err.contains("a"));
}

#[test]
fn validate_detects_three_node_cycle() {
    let mut graph = DependencyGraph::new();
    // a -> b -> c -> a
    graph.add_dependency("a", "c");
    graph.add_dependency("b", "a");
    graph.add_dependency("c", "b");

    let err = graph.validate().unwrap_err();
    assert!(err.contains("cycle"));
    // All three nodes should be reported as offenders.
    assert!(err.contains("a") && err.contains("b") && err.contains("c"));
}

#[test]
fn validate_detects_cycle_with_acyclic_prefix() {
    let mut graph = DependencyGraph::new();
    // Acyclic prefix: root -> mid
    graph.add_dependency("mid", "root");
    // Cyclic suffix: mid <-> tail (tail depends on mid, mid now also depends on tail)
    graph.add_dependency("tail", "mid");
    graph.add_dependency("mid", "tail");

    let err = graph.validate().unwrap_err();
    assert!(err.contains("cycle"));
    assert!(err.contains("mid") && err.contains("tail"));
    // `root` is clean — the acyclic prefix should not be in the offenders.
    assert!(
        !err.contains("root"),
        "clean prefix must not be reported as part of the cycle: {err}"
    );
}

#[test]
fn validate_error_lists_offenders_deterministically() {
    // Run the same cyclic input multiple times — error message must
    // be byte-identical so ops/logging can dedupe it.
    let make_cycle = || {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("x", "y");
        graph.add_dependency("y", "z");
        graph.add_dependency("z", "x");
        graph.validate().unwrap_err()
    };
    let e1 = make_cycle();
    let e2 = make_cycle();
    let e3 = make_cycle();
    assert_eq!(e1, e2);
    assert_eq!(e2, e3);
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

// ── executor scaffold status ───────────────────────────────────
//
// `ParallelExecutor` previously returned `Ok(statuses)` after
// spawning tasks that slept 100ms and lied about completion. These
// tests lock in the new behaviour: execution is an explicit
// `NotImplemented` error so callers cannot mistake the scaffold for
// working code.

#[tokio::test]
async fn test_executor_execute_plan_returns_not_implemented() {
    use super::executor::{ParallelExecutionError, ParallelExecutor};
    use crate::pipeline::queue::TaskQueue;
    use crate::pipeline::worker_pool::{WorkerPool, WorkerPoolConfig};

    let worker_pool = std::sync::Arc::new(WorkerPool::new(WorkerPoolConfig::default()));
    let queue = std::sync::Arc::new(TaskQueue::new());
    let executor = ParallelExecutor::new(worker_pool, queue, 4);

    let task = Task::new("TASK-001", "Parent", "Parent instruction");
    let child = ChildTaskDefinition {
        key: "child-a".to_string(),
        title: "Child A".to_string(),
        instruction: "Do A".to_string(),
        complexity: 0.5,
        depends_on: vec![],
        estimated_duration: None,
        tags: vec![],
        priority: None,
        initial_phase: None,
    };
    let decomposer = TaskDecomposer::new();
    let plan = decomposer.create_plan(&task, vec![child]);

    let err = executor
        .execute_plan(&plan)
        .await
        .expect_err("scaffold must return an explicit error");

    match err {
        ParallelExecutionError::NotImplemented(msg) => {
            assert!(
                msg.contains("scaffold"),
                "error message must identify the scaffold status; got: {msg}"
            );
        }
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

