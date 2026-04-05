//! Scope Resolver Tests

use std::path::PathBuf;

use super::resolver::{find_repo_root, TaskScope};
use super::types::ScopeMode;

#[test]
fn test_scope_mode_display() {
    assert_eq!(ScopeMode::Repo.to_string(), "repo");
    assert_eq!(ScopeMode::Subdir.to_string(), "subdir");
    assert_eq!(ScopeMode::NestedRepo.to_string(), "nested_repo");
    assert_eq!(ScopeMode::MultiRepo.to_string(), "multi_repo");
}

#[test]
fn test_find_repo_root() {
    let temp = tempfile::tempdir().unwrap();
    let repo_path = temp.path().to_path_buf();
    std::fs::create_dir(repo_path.join(".git")).unwrap();

    let sub_path = repo_path.join("src/components");
    std::fs::create_dir_all(&sub_path).unwrap();

    let root = find_repo_root(&sub_path);
    // Should find the temp repo root
    assert_eq!(
        root.unwrap().canonicalize().unwrap(),
        repo_path.canonicalize().unwrap()
    );
}

#[test]
fn test_task_scope_repo_wide() {
    let scope = TaskScope::repo_wide(PathBuf::from("/project"));
    assert_eq!(scope.scope_mode, ScopeMode::Repo);
    assert!(!scope.allow_scope_expansion);
}

#[test]
fn test_task_scope_subdir() {
    let scope = TaskScope::subdir(PathBuf::from("/project"), PathBuf::from("src/components"));
    assert_eq!(scope.scope_mode, ScopeMode::Subdir);
    assert!(scope.allow_scope_expansion);
}

#[test]
fn test_suggested_branch_name() {
    let scope = TaskScope::repo_wide(PathBuf::from("/project"));
    let branch = scope.suggested_branch_name("TASK-001");
    assert_eq!(branch, "d3vx/TASK-001");
}

#[test]
fn test_is_path_allowed() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().to_path_buf().canonicalize().unwrap();
    let src_path = project_root.join("src");
    std::fs::create_dir(&src_path).unwrap();

    let main_rs = src_path.join("main.rs");
    std::fs::write(&main_rs, "").unwrap();

    let mut scope = TaskScope::repo_wide(project_root.clone());
    scope.task_scope_path = src_path.clone();

    assert!(scope.is_path_allowed(&main_rs));

    let other = project_root.join("other.rs");
    std::fs::write(&other, "").unwrap();
    assert!(!scope.is_path_allowed(&other));
}
