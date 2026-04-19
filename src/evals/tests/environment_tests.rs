//! Tests for eval environment provisioning and cleanup.

use std::fs;
use std::path::PathBuf;

use crate::evals::environment::{EnvironmentError, EvalEnvironment};
use crate::evals::task::EvalTask;

fn tmp_root(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-evals-env-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn bare_task(id: &str, setup: Vec<String>) -> EvalTask {
    EvalTask {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        instruction: "do".to_string(),
        setup,
        graders: Vec::new(),
        budget_usd: None,
        max_iterations: None,
        timeout_secs: None,
        tags: Vec::new(),
    }
}

#[test]
fn provision_creates_unique_workspace_inside_root() {
    let root = tmp_root("provision_ok");
    let task = bare_task("basic", Vec::new());
    let env = EvalEnvironment::provision(&task, &root).unwrap();
    assert!(env.workspace_path.exists());
    assert!(env.workspace_path.starts_with(&root));
    assert!(env.id.starts_with("basic-"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn provision_runs_setup_steps_in_order_inside_workspace() {
    let root = tmp_root("setup_ok");
    let task = bare_task(
        "seeds",
        vec![
            "echo one > 1.txt".to_string(),
            "echo two > 2.txt".to_string(),
        ],
    );
    let env = EvalEnvironment::provision(&task, &root).unwrap();
    assert!(env.workspace_path.join("1.txt").exists());
    assert!(env.workspace_path.join("2.txt").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn provision_fails_when_setup_step_returns_nonzero() {
    let root = tmp_root("setup_fail");
    let task = bare_task(
        "bad",
        vec![
            "true".to_string(),
            "false".to_string(),
            "touch should-not-exist.txt".to_string(),
        ],
    );
    let err = EvalEnvironment::provision(&task, &root).unwrap_err();
    match err {
        EnvironmentError::SetupFailed { step_index, .. } => {
            assert_eq!(step_index, 1);
        }
        other => panic!("expected SetupFailed, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn two_provisions_do_not_collide() {
    let root = tmp_root("collision");
    let task = bare_task("same-id", Vec::new());
    let a = EvalEnvironment::provision(&task, &root).unwrap();
    let b = EvalEnvironment::provision(&task, &root).unwrap();
    assert_ne!(a.workspace_path, b.workspace_path);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cleanup_removes_workspace_directory() {
    let root = tmp_root("cleanup_ok");
    let task = bare_task("cleanable", Vec::new());
    let env = EvalEnvironment::provision(&task, &root).unwrap();
    let path = env.workspace_path.clone();
    assert!(path.exists());
    env.cleanup().unwrap();
    assert!(!path.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cleanup_is_idempotent_when_path_missing() {
    let root = tmp_root("cleanup_idem");
    let task = bare_task("x", Vec::new());
    let env = EvalEnvironment::provision(&task, &root).unwrap();
    let path = env.workspace_path.clone();
    fs::remove_dir_all(&path).unwrap(); // delete out from under cleanup
    let env = EvalEnvironment::adopt("x", path);
    env.cleanup().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn adopt_creates_env_without_provisioning() {
    let root = tmp_root("adopt");
    fs::create_dir_all(root.join("adopted")).unwrap();
    let env = EvalEnvironment::adopt("ad", root.join("adopted"));
    assert_eq!(env.id, "ad");
    assert!(env.env_vars.is_empty());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn with_env_builder_accumulates_variables() {
    let root = tmp_root("with_env");
    fs::create_dir_all(&root).unwrap();
    let env = EvalEnvironment::adopt("w", root.join("ws"))
        .with_env("A", "1")
        .with_env("B", "2");
    assert_eq!(env.env_vars.get("A").map(|s| s.as_str()), Some("1"));
    assert_eq!(env.env_vars.get("B").map(|s| s.as_str()), Some("2"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn provision_creates_root_if_missing() {
    let root = std::env::temp_dir().join(format!(
        "d3vx-evals-auto-root-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(!root.exists());
    let task = bare_task("auto", Vec::new());
    let env = EvalEnvironment::provision(&task, &root).unwrap();
    assert!(root.exists());
    env.cleanup().unwrap();
    fs::remove_dir_all(&root).unwrap();
}
