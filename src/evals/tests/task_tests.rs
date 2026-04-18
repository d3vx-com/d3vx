//! Tests for task definition, validation, and TOML loading.

use std::fs;
use std::path::PathBuf;

use crate::evals::grader::GraderSpec;
use crate::evals::task::{EvalTask, TaskError, TaskLoadError};

fn tmp_root(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-evals-task-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn minimal_task() -> EvalTask {
    EvalTask {
        id: "t".to_string(),
        name: "test".to_string(),
        description: None,
        instruction: "do a thing".to_string(),
        setup: Vec::new(),
        graders: Vec::new(),
        budget_usd: None,
        max_iterations: None,
        timeout_secs: None,
        tags: Vec::new(),
    }
}

#[test]
fn validate_accepts_minimal_task() {
    assert!(minimal_task().validate().is_ok());
}

#[test]
fn validate_rejects_empty_name() {
    let mut t = minimal_task();
    t.name = String::new();
    match t.validate() {
        Err(TaskError::EmptyField("name")) => {}
        other => panic!("expected EmptyField(name), got {other:?}"),
    }
}

#[test]
fn validate_rejects_empty_instruction() {
    let mut t = minimal_task();
    t.instruction = String::new();
    match t.validate() {
        Err(TaskError::EmptyField("instruction")) => {}
        other => panic!("expected EmptyField(instruction), got {other:?}"),
    }
}

#[test]
fn validate_rejects_zero_max_iterations() {
    let mut t = minimal_task();
    t.max_iterations = Some(0);
    assert!(matches!(
        t.validate(),
        Err(TaskError::NonPositive { field: "max_iterations", .. })
    ));
}

#[test]
fn validate_rejects_zero_timeout() {
    let mut t = minimal_task();
    t.timeout_secs = Some(0);
    assert!(matches!(
        t.validate(),
        Err(TaskError::NonPositive { field: "timeout_secs", .. })
    ));
}

#[test]
fn validate_rejects_zero_budget() {
    let mut t = minimal_task();
    t.budget_usd = Some(0.0);
    assert!(matches!(
        t.validate(),
        Err(TaskError::NonPositive { field: "budget_usd", .. })
    ));
}

#[test]
fn validate_rejects_nan_budget() {
    let mut t = minimal_task();
    t.budget_usd = Some(f64::NAN);
    assert!(matches!(
        t.validate(),
        Err(TaskError::NonPositive { field: "budget_usd", .. })
    ));
}

#[test]
fn display_name_falls_back_to_id_when_name_empty() {
    let mut t = minimal_task();
    t.name = String::new();
    t.id = "the-id".to_string();
    assert_eq!(t.display_name(), "the-id");
}

#[test]
fn has_tag_is_case_sensitive() {
    let mut t = minimal_task();
    t.tags = vec!["Bugfix".to_string()];
    assert!(t.has_tag("Bugfix"));
    assert!(!t.has_tag("bugfix"));
}

#[test]
fn load_from_file_parses_minimal_task_and_derives_id() {
    let dir = tmp_root("load_min");
    let path = dir.join("fix-login.toml");
    fs::write(
        &path,
        r#"
name = "Fix login bug"
instruction = "Repair the broken login handler."
"#,
    )
    .unwrap();

    let t = EvalTask::load_from_file(&path).unwrap();
    assert_eq!(t.id, "fix-login");
    assert_eq!(t.name, "Fix login bug");
    assert_eq!(t.instruction, "Repair the broken login handler.");
    assert!(t.graders.is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_file_honours_explicit_id() {
    let dir = tmp_root("load_expl_id");
    let path = dir.join("anything.toml");
    fs::write(
        &path,
        r#"
id = "explicit-id"
name = "task"
instruction = "do"
"#,
    )
    .unwrap();
    let t = EvalTask::load_from_file(&path).unwrap();
    assert_eq!(t.id, "explicit-id");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_file_parses_graders() {
    let dir = tmp_root("load_graders");
    let path = dir.join("graded.toml");
    fs::write(
        &path,
        r#"
name = "graded"
instruction = "do"
setup = ["echo hi"]

[[graders]]
type = "file_exists"
path = "README.md"

[[graders]]
type = "shell_command"
command = "cargo check"
"#,
    )
    .unwrap();
    let t = EvalTask::load_from_file(&path).unwrap();
    assert_eq!(t.setup, vec!["echo hi".to_string()]);
    assert_eq!(t.graders.len(), 2);
    assert!(matches!(t.graders[0], GraderSpec::FileExists { .. }));
    assert!(matches!(t.graders[1], GraderSpec::ShellCommand { .. }));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_file_surfaces_parse_error_with_path() {
    let dir = tmp_root("bad");
    let path = dir.join("broken.toml");
    fs::write(&path, "this is not valid TOML = =").unwrap();
    let err = EvalTask::load_from_file(&path).unwrap_err();
    match err {
        TaskLoadError::Parse { path: p, .. } => {
            assert_eq!(p, path);
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_file_rejects_task_that_fails_validation() {
    let dir = tmp_root("invalid");
    let path = dir.join("empty-instr.toml");
    fs::write(&path, "name = \"a\"\ninstruction = \"\"\n").unwrap();
    let err = EvalTask::load_from_file(&path).unwrap_err();
    assert!(matches!(err, TaskLoadError::Invalid { .. }));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_dir_skips_non_toml_and_sorts_deterministically() {
    let dir = tmp_root("dir_load");
    fs::write(
        dir.join("c-task.toml"),
        "name = \"c\"\ninstruction = \"do c\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("a-task.toml"),
        "name = \"a\"\ninstruction = \"do a\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("b-task.toml"),
        "name = \"b\"\ninstruction = \"do b\"\n",
    )
    .unwrap();
    fs::write(dir.join("README.md"), "ignore me").unwrap();

    let results = EvalTask::load_from_dir(&dir).unwrap();
    let tasks: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].id, "a-task");
    assert_eq!(tasks[1].id, "b-task");
    assert_eq!(tasks[2].id, "c-task");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_dir_returns_per_file_errors_without_failing_batch() {
    let dir = tmp_root("mixed_dir");
    fs::write(
        dir.join("ok.toml"),
        "name = \"ok\"\ninstruction = \"do\"\n",
    )
    .unwrap();
    fs::write(dir.join("bad.toml"), "not toml = =").unwrap();

    let results = EvalTask::load_from_dir(&dir).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.is_ok()));
    assert!(results.iter().any(|r| r.is_err()));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_from_dir_errors_when_directory_missing() {
    let fake = std::env::temp_dir().join("d3vx-evals-does-not-exist-zzz");
    assert!(EvalTask::load_from_dir(&fake).is_err());
}
