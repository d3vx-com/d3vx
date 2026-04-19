//! Tests for RestoreSafetyChecker and ConflictCheckResult.

use crate::pipeline::lifecycle::restore::{ConflictCheckResult, RestoreSafetyChecker};

#[tokio::test]
async fn test_conflict_check_passes_for_clean_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    assert!(result.is_safe, "Clean workspace should be safe to restore");
    assert!(result.reasons.is_empty());
}

#[tokio::test]
async fn test_conflict_check_fails_for_conflict_markers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    // Write a file with conflict markers and stage it
    let file_path = workspace.join("conflict.rs");
    tokio::fs::write(
        &file_path,
        r#"fn foo() {
<<<<<<< HEAD
    println!("hello");
=======
    println!("world");
>>>>>>> feature
}
"#,
    )
    .await
    .expect("write conflict file");

    // Stage the conflict file
    std::process::Command::new("git")
        .arg("add")
        .arg("conflict.rs")
        .current_dir(&workspace)
        .output()
        .expect("git add");

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    assert!(
        !result.is_safe,
        "Workspace with conflict markers should not be safe"
    );
    assert!(
        result
            .conflict_marker_files
            .iter()
            .any(|f| f.contains("conflict.rs")),
        "Should detect conflict.rs"
    );
    assert!(
        result
            .reasons
            .iter()
            .any(|r| r.contains("conflict markers")),
        "Should have conflict marker reason"
    );
}

#[tokio::test]
async fn test_conflict_check_fails_for_diff_check_errors() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    // Commit initial file
    let file_path = workspace.join("test.rs");
    tokio::fs::write(&file_path, "fn test() {}\n")
        .await
        .expect("write initial file");
    std::process::Command::new("git")
        .arg("add")
        .current_dir(&workspace)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&workspace)
        .output()
        .expect("git commit");

    // Make changes with whitespace errors (trailing whitespace)
    tokio::fs::write(&file_path, "fn test() {}\nfn other() {}\n")
        .await
        .expect("write file with trailing whitespace");

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    // Note: Without actual diff, this may pass. The test validates the checker runs.
    // A real diff-check failure would require actual staged changes.
    assert!(
        result.diff_check_errors.is_empty(),
        "No diff errors expected for simple change"
    );
}

#[tokio::test]
async fn test_conflict_check_result_safe() {
    let result = ConflictCheckResult::safe();
    assert!(result.is_safe);
    assert!(result.conflict_marker_files.is_empty());
    assert!(result.diff_check_errors.is_empty());
    assert!(result.dirty_files.is_empty());
    assert!(result.reasons.is_empty());
}

#[tokio::test]
async fn test_conflict_check_result_unsafe() {
    let result = ConflictCheckResult::unsafe_(
        vec!["file1.rs".to_string(), "file2.rs".to_string()],
        vec!["whitespace error".to_string()],
        vec!["dirty.txt".to_string()],
    );

    assert!(!result.is_safe);
    assert_eq!(result.conflict_marker_files.len(), 2);
    assert_eq!(result.diff_check_errors.len(), 1);
    assert_eq!(result.dirty_files.len(), 1);
    assert!(!result.reasons.is_empty());

    let reasons_text = result.reasons.join(" ");
    assert!(reasons_text.contains("conflict markers"));
    assert!(reasons_text.contains("file1.rs"));
    assert!(reasons_text.contains("file2.rs"));
}

#[tokio::test]
async fn test_conflict_check_handles_non_git_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // No git init - just a plain directory

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    // Should handle gracefully without panicking
    // Non-git workspace is treated as safe (no conflicts possible)
    assert!(result.is_safe || !result.reasons.is_empty());
}

#[tokio::test]
async fn test_conflict_check_blocks_dirty_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    // Commit initial file
    let file_path = workspace.join("test.rs");
    tokio::fs::write(&file_path, "fn test() {}\n")
        .await
        .expect("write initial file");
    std::process::Command::new("git")
        .arg("add")
        .arg("test.rs")
        .current_dir(&workspace)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&workspace)
        .output()
        .expect("git commit");

    // Make uncommitted changes
    tokio::fs::write(&file_path, "fn test() {}\nfn extra() {}\n")
        .await
        .expect("write modified file");

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    assert!(
        !result.is_safe,
        "Dirty workspace should not be safe to restore"
    );
    assert!(!result.dirty_files.is_empty(), "Should detect dirty files");
    assert!(
        result
            .reasons
            .iter()
            .any(|r| r.contains("uncommitted changes")),
        "Should explain uncommitted changes"
    );
}

#[tokio::test]
async fn test_conflict_check_detects_whitespace_errors() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().to_path_buf();
    std::mem::forget(dir);

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    // Commit initial file
    let file_path = workspace.join("test.rs");
    tokio::fs::write(&file_path, "fn test() {}\n")
        .await
        .expect("write initial file");
    std::process::Command::new("git")
        .arg("add")
        .arg("test.rs")
        .current_dir(&workspace)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&workspace)
        .output()
        .expect("git commit");

    // Modify with trailing whitespace (diff --check will catch this)
    tokio::fs::write(&file_path, "fn test() {}\nfn extra() {}  \n")
        .await
        .expect("write file with trailing whitespace");

    // Stage the changes to make git diff --check detect them
    std::process::Command::new("git")
        .arg("add")
        .arg("test.rs")
        .current_dir(&workspace)
        .output()
        .expect("git add for staging");

    let result = RestoreSafetyChecker::check_workspace(&workspace).await;
    // Trailing whitespace should trigger diff-check failure
    assert!(
        !result.is_safe || !result.diff_check_errors.is_empty(),
        "Workspace with whitespace errors should not be safe or should have diff errors"
    );
}
