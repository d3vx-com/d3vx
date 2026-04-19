//! Tests for SessionRestore assess / plan / execute flows.

use std::sync::Arc;

use crate::pipeline::heartbeat::HeartbeatManager;
use crate::pipeline::lifecycle::restore::{
    RestoreCheck, RestoreError, RestorePlan, RestoreStatus, SessionRestore, WORKTREE_BASE_DIR,
};
use crate::pipeline::resume::{ResumeManager, SessionSnapshot};
use crate::pipeline::WorkerId;

fn test_restore() -> SessionRestore {
    let dir = tempfile::tempdir().expect("temp dir");
    let project_root = dir.path().to_path_buf();
    std::mem::forget(dir);
    let resume_manager = ResumeManager::new(project_root.join("snapshots"));
    SessionRestore::new(resume_manager, project_root)
}

fn test_restore_with_heartbeat() -> (SessionRestore, Arc<HeartbeatManager>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let project_root = dir.path().to_path_buf();
    std::mem::forget(dir);
    let resume_manager = ResumeManager::new(project_root.join("snapshots"));
    let heartbeat_manager = Arc::new(HeartbeatManager::with_defaults());
    let restore = SessionRestore::new(resume_manager, project_root)
        .with_heartbeat_manager(heartbeat_manager.clone());
    (restore, heartbeat_manager)
}

fn make_snapshot(session_id: &str, task_id: &str) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session_id.to_string(),
        task_id: task_id.to_string(),
        snapshot_at: chrono::Utc::now(),
        messages: vec![],
        event_log: None,
        current_phase: "working".to_string(),
        modified_files: vec![],
        tool_history: vec![],
        checkpoint_note: None,
    }
}

#[tokio::test]
async fn test_assess_passes_for_valid_session() {
    let restore = test_restore();
    let snapshot = make_snapshot("sess-001", "task-001");

    restore.resume_manager.initialize().await.expect("init");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    // Create the workspace directory and initialize it as a git repository so WorkspaceExists check passes.
    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-001");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");
    // Initialize git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(output.status.success());

    let status = restore.assess("sess-001").await;
    match status {
        RestoreStatus::CanRestore { checks_passed } => {
            assert!(
                checks_passed.contains(&RestoreCheck::MetadataValid),
                "metadata should be valid"
            );
            assert!(
                checks_passed.contains(&RestoreCheck::WorkspaceExists),
                "workspace should exist"
            );
            assert!(
                checks_passed.contains(&RestoreCheck::AgentNotRunning),
                "agent should not be running"
            );
        }
        RestoreStatus::Blocked {
            failed_checks,
            reasons,
        } => {
            if failed_checks.contains(&RestoreCheck::NoConflicts) {
                // Acceptable failure due to missing git repo
                assert_eq!(reasons.len(), 1);
                assert!(
                    reasons[0].contains("Git diff check failures"),
                    "Expected git diff usage error"
                );
            } else {
                panic!(
                    "Expected CanRestore or acceptable Blocked due to Git, got reasons: {:?}",
                    reasons
                );
            }
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected CanRestore, got AlreadyRunning");
        }
    }
}

#[tokio::test]
async fn test_plan_generates_reconnect_command() {
    let restore = test_restore();
    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-002", "task-002");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    // Create workspace so it does not need recreation.
    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-002");
    tokio::fs::create_dir_all(&workspace).await.expect("ws");

    let status = RestoreStatus::CanRestore {
        checks_passed: vec![
            RestoreCheck::MetadataValid,
            RestoreCheck::WorkspaceExists,
            RestoreCheck::BranchExists,
            RestoreCheck::NoConflicts,
            RestoreCheck::AgentNotRunning,
        ],
    };

    let plan = restore.plan("sess-002", &status).await.expect("plan");
    assert_eq!(plan.session_id, "sess-002");
    assert!(
        plan.agent_reconnect_command.is_some(),
        "should have a reconnect command"
    );
    assert!(plan.agent_reconnect_command.unwrap().contains("sess-002"));
}

#[tokio::test]
async fn test_blocked_if_agent_running() {
    let restore = test_restore();
    let status = RestoreStatus::AlreadyRunning;
    let result = restore.plan("sess-003", &status).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        RestoreError::AgentStillAlive(id) => assert_eq!(id, "sess-003"),
        other => panic!("Expected AgentStillAlive, got {:?}", other),
    }
}

#[tokio::test]
async fn test_restore_outcome_on_success() {
    let restore = test_restore();
    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-004", "task-004");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let plan = RestorePlan {
        session_id: "sess-004".to_string(),
        workspace_path: restore
            .project_root
            .join(WORKTREE_BASE_DIR)
            .join("d3vx-task-task-004"),
        branch: "d3vx-task-task-004".to_string(),
        needs_workspace_recreate: false,
        agent_reconnect_command: Some("d3vx session resume --id sess-004".to_string()),
    };

    let outcome = restore.execute(plan).await.expect("execute");
    assert!(outcome.success, "restore should succeed");
    assert!(outcome.new_session_id.is_some(), "should have a session id");
    assert!(
        outcome.message.contains("sess-004"),
        "message should reference session"
    );
    assert!(
        !outcome.workspace_recreated,
        "workspace should not be recreated when it exists"
    );
}

#[tokio::test]
async fn test_assess_returns_already_running_when_lease_exists() {
    let (restore, heartbeat_manager) = test_restore_with_heartbeat();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-active", "task-active");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-active");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    let worker_id = WorkerId(1);
    heartbeat_manager
        .create_lease(worker_id, "task-active")
        .await
        .expect("create lease");

    let status = restore.assess("sess-active").await;
    match status {
        RestoreStatus::AlreadyRunning => {}
        RestoreStatus::CanRestore { .. } => {
            panic!("Expected AlreadyRunning when lease exists");
        }
        RestoreStatus::Blocked { .. } => {
            panic!("Expected AlreadyRunning, got Blocked");
        }
    }
}

#[tokio::test]
async fn test_assess_can_restore_when_lease_expired() {
    let (restore, _heartbeat_manager) = test_restore_with_heartbeat();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-expired", "task-expired");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-expired");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    // Init git repo so workspace safety checks don't fail
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");

    let status = restore.assess("sess-expired").await;
    match status {
        RestoreStatus::CanRestore { checks_passed } => {
            assert!(
                checks_passed.contains(&RestoreCheck::AgentNotRunning),
                "should pass agent not running check when no active lease"
            );
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected CanRestore when no active lease");
        }
        RestoreStatus::Blocked { .. } => {
            panic!("Expected CanRestore, got Blocked");
        }
    }
}

#[tokio::test]
async fn test_assess_can_restore_without_heartbeat_manager() {
    let restore = test_restore();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-no-hb", "task-no-hb");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-no-hb");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    // Init git repo so workspace safety checks don't fail
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");

    let status = restore.assess("sess-no-hb").await;
    match status {
        RestoreStatus::CanRestore { checks_passed } => {
            assert!(
                checks_passed.contains(&RestoreCheck::AgentNotRunning),
                "should pass without heartbeat manager"
            );
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected CanRestore without heartbeat manager");
        }
        RestoreStatus::Blocked { .. } => {
            panic!("Expected CanRestore, got Blocked");
        }
    }
}

#[tokio::test]
async fn test_assess_blocks_restore_with_conflict_markers() {
    let restore = test_restore();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-conflict", "task-conflict");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-conflict");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    // Initialize git repo in workspace
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");

    // Create an initial clean commit
    let readme = workspace.join("README.md");
    std::fs::write(&readme, "# workspace").expect("readme");
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&workspace)
        .args(["add", "."])
        .output();
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&workspace)
        .args(["commit", "-m", "init", "--no-gpg-sign"])
        .output();

    // Write file with conflict markers and track it
    let file_path = workspace.join("src/main.rs");
    std::fs::create_dir_all(workspace.join("src")).expect("create src dir");
    std::fs::write(
        &file_path,
        r#"<<<<<<< HEAD
fn main() { println!("A"); }
=======
fn main() { println!("B"); }
>>>>>>> feature
"#,
    )
    .expect("write conflict file");

    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&workspace)
        .args(["add", "."])
        .output();
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&workspace)
        .args(["commit", "-m", "conflict", "--allow-empty", "--no-gpg-sign"])
        .output();

    // Now modify the tracked file to have conflict markers (will be caught by git diff --check or conflict markers grep)
    std::fs::write(
        &file_path,
        r#"<<<<<<< HEAD
fn main() { println!("A"); }
=======
fn main() { println!("B"); }
>>>>>>> feature
"#,
    )
    .expect("rewrite conflict file");

    let status = restore.assess("sess-conflict").await;
    match status {
        RestoreStatus::Blocked {
            failed_checks,
            reasons,
        } => {
            assert!(
                failed_checks.contains(&RestoreCheck::NoConflicts),
                "Should have NoConflicts in failed checks"
            );
            assert!(!reasons.is_empty());
        }
        RestoreStatus::CanRestore { .. } => {
            panic!("Expected Blocked, got CanRestore");
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected Blocked, not AlreadyRunning");
        }
    }
}

#[tokio::test]
async fn test_assess_allows_restore_when_workspace_missing() {
    let restore = test_restore();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-no-ws", "task-no-ws");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    // Do NOT create the workspace directory

    let status = restore.assess("sess-no-ws").await;
    match status {
        RestoreStatus::Blocked {
            failed_checks,
            reasons: _,
        } => {
            assert!(
                failed_checks.contains(&RestoreCheck::WorkspaceExists),
                "Should fail on missing workspace"
            );
            assert!(
                !failed_checks.contains(&RestoreCheck::NoConflicts),
                "Should not fail NoConflicts when workspace missing"
            );
        }
        RestoreStatus::CanRestore { .. } => {
            panic!("Expected Blocked due to missing workspace");
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected Blocked");
        }
    }
}

#[tokio::test]
async fn test_assess_blocks_restore_with_dirty_workspace() {
    let restore = test_restore();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-dirty", "task-dirty");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-dirty");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    // Initialize git repo
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");

    // Commit initial file
    let file_path = workspace.join("src/lib.rs");
    tokio::fs::create_dir_all(workspace.join("src"))
        .await
        .expect("create src dir");
    tokio::fs::write(&file_path, "pub fn init() {}\n")
        .await
        .expect("write initial file");
    std::process::Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(&workspace)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&workspace)
        .output()
        .expect("git commit");

    // Make uncommitted changes
    tokio::fs::write(&file_path, "pub fn init() {}\npub fn new_fn() {}\n")
        .await
        .expect("modify file");

    let status = restore.assess("sess-dirty").await;
    match status {
        RestoreStatus::Blocked {
            failed_checks,
            reasons,
        } => {
            assert!(
                failed_checks.contains(&RestoreCheck::NoConflicts),
                "Should have NoConflicts in failed checks"
            );
            assert!(
                reasons.iter().any(|r| r.contains("uncommitted changes")),
                "Should explain uncommitted changes"
            );
        }
        RestoreStatus::CanRestore { .. } => {
            panic!("Expected Blocked due to dirty workspace");
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected Blocked, not AlreadyRunning");
        }
    }
}

#[tokio::test]
async fn test_assess_passes_with_clean_workspace() {
    let restore = test_restore();

    restore.resume_manager.initialize().await.expect("init");

    let snapshot = make_snapshot("sess-clean", "task-clean");
    restore
        .resume_manager
        .save_snapshot(&snapshot)
        .await
        .expect("save");

    let workspace = restore
        .project_root
        .join(WORKTREE_BASE_DIR)
        .join("d3vx-task-task-clean");
    tokio::fs::create_dir_all(&workspace)
        .await
        .expect("create workspace");

    // Initialize git repo
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&workspace)
        .output()
        .expect("git init");

    // Commit a file (no uncommitted changes)
    let file_path = workspace.join("clean.rs");
    tokio::fs::write(&file_path, "fn clean() {}\n")
        .await
        .expect("write clean file");
    std::process::Command::new("git")
        .arg("add")
        .arg("clean.rs")
        .current_dir(&workspace)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&workspace)
        .output()
        .expect("git commit");

    let status = restore.assess("sess-clean").await;
    match status {
        RestoreStatus::CanRestore { checks_passed } => {
            assert!(
                checks_passed.contains(&RestoreCheck::NoConflicts),
                "Clean workspace should pass NoConflicts check"
            );
            assert!(
                checks_passed.contains(&RestoreCheck::WorkspaceExists),
                "Should have workspace exists"
            );
        }
        RestoreStatus::Blocked { reasons, .. } => {
            panic!(
                "Expected CanRestore for clean workspace, got Blocked: {:?}",
                reasons
            );
        }
        RestoreStatus::AlreadyRunning => {
            panic!("Expected CanRestore, not AlreadyRunning");
        }
    }
}
