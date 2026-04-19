//! Tests for the eval runner, exercised via a mock driver so the
//! framework is independently verifiable without a real agent loop.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::evals::grader::GraderSpec;
use crate::evals::runner::EvalRunner;
use crate::evals::task::EvalTask;
use crate::evals::tests::runner_helpers::{
    CountingDriver, CreateFileDriver, FailingDriver, SleepingDriver,
};

fn tmp_root(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-evals-runner-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn task_passing(id: &str) -> EvalTask {
    // Grader: file `ok.txt` exists in workspace. We let the mock driver
    // create it.
    EvalTask {
        id: id.to_string(),
        name: format!("task {id}"),
        description: None,
        instruction: "create ok.txt".to_string(),
        setup: Vec::new(),
        graders: vec![GraderSpec::FileExists {
            path: "ok.txt".to_string(),
        }],
        budget_usd: None,
        max_iterations: None,
        timeout_secs: None,
        tags: Vec::new(),
    }
}

fn task_timeout(id: &str, secs: u64) -> EvalTask {
    EvalTask {
        id: id.to_string(),
        name: format!("slow {id}"),
        description: None,
        instruction: "do nothing but sleep".to_string(),
        setup: Vec::new(),
        graders: vec![GraderSpec::FileExists {
            path: "never.txt".to_string(),
        }],
        budget_usd: None,
        max_iterations: None,
        timeout_secs: Some(secs),
        tags: Vec::new(),
    }
}

#[tokio::test]
async fn run_passes_when_driver_satisfies_graders() {
    let root = tmp_root("pass");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = CreateFileDriver {
        file_name: "ok.txt".to_string(),
        cost: 0.12,
        iterations: 3,
        tool_calls: 5,
    };

    let result = runner.run(&task, &driver).await;
    assert!(result.passed, "task should pass");
    assert_eq!(result.cost_usd, Some(0.12));
    assert_eq!(result.iterations, Some(3));
    assert_eq!(result.tool_calls, Some(5));
    assert!(result.harness_error.is_none());
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_fails_when_driver_produces_wrong_artifact() {
    let root = tmp_root("wrongfile");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = CreateFileDriver {
        file_name: "other.txt".to_string(),
        cost: 0.0,
        iterations: 0,
        tool_calls: 0,
    };

    let result = runner.run(&task, &driver).await;
    assert!(!result.passed, "grader should reject");
    assert!(result.harness_error.is_none());
    assert_eq!(result.grader_outcomes.len(), 1);
    assert!(!result.grader_outcomes[0].passed);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_reports_harness_failure_when_driver_errors() {
    let root = tmp_root("driver_err");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = FailingDriver {
        message: "provider timed out".to_string(),
    };

    let result = runner.run(&task, &driver).await;
    assert!(!result.passed);
    let err = result.harness_error.expect("harness error should be set");
    assert!(err.contains("provider timed out"));
    assert!(result.grader_outcomes.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_enforces_task_timeout() {
    let root = tmp_root("timeout");
    let runner = EvalRunner::new(&root);
    let task = task_timeout("slow", 1);
    let driver = SleepingDriver;

    let started = std::time::Instant::now();
    let result = runner.run(&task, &driver).await;
    let elapsed = started.elapsed();

    assert!(!result.passed);
    assert!(elapsed.as_secs() < 5, "timeout must fire promptly, got {elapsed:?}");
    let err = result.harness_error.expect("timeout yields harness error");
    assert!(err.contains("timed out"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_batch_executes_every_task_and_collects_report() {
    let root = tmp_root("batch");
    let runner = EvalRunner::new(&root);
    let calls = Arc::new(AtomicU32::new(0));
    let driver = CountingDriver {
        calls: calls.clone(),
    };

    let tasks = vec![task_passing("a"), task_passing("b"), task_passing("c")];
    let report = runner.run_batch(&tasks, &driver).await;

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(report.results.len(), 3);
    assert_eq!(report.passed_count(), 3);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_records_grader_outcome_details() {
    let root = tmp_root("details");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = CreateFileDriver {
        file_name: "ok.txt".to_string(),
        cost: 0.0,
        iterations: 0,
        tool_calls: 0,
    };

    let result = runner.run(&task, &driver).await;
    assert_eq!(result.grader_outcomes.len(), 1);
    let o = &result.grader_outcomes[0];
    assert!(o.passed);
    assert!(o.detail.contains("ok.txt"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_cleans_up_workspace_on_pass_by_default() {
    let root = tmp_root("cleanup_pass");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = CreateFileDriver {
        file_name: "ok.txt".to_string(),
        cost: 0.0,
        iterations: 0,
        tool_calls: 0,
    };

    runner.run(&task, &driver).await;
    // Workspace for a passing task is removed. The root may still
    // exist; we just check no per-task subdirs are left behind.
    let remaining: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert!(
        remaining.is_empty(),
        "passing run should leave no workspace behind"
    );
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn run_keeps_workspace_on_failure_by_default() {
    let root = tmp_root("keep_fail");
    let runner = EvalRunner::new(&root);
    let task = task_passing("t1");
    let driver = FailingDriver {
        message: "boom".to_string(),
    };

    runner.run(&task, &driver).await;
    let remaining: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert_eq!(
        remaining.len(),
        1,
        "failed run workspace should be preserved for inspection"
    );
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn keep_on_failure_false_removes_workspace_on_failure() {
    let root = tmp_root("nokeep");
    let runner = EvalRunner::new(&root).keep_on_failure(false);
    let task = task_passing("t1");
    let driver = FailingDriver {
        message: "boom".to_string(),
    };

    runner.run(&task, &driver).await;
    let remaining: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert!(
        remaining.is_empty(),
        "with keep_on_failure(false), failed run must clean up"
    );
    fs::remove_dir_all(&root).ok();
}
