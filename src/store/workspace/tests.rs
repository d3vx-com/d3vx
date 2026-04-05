//! Workspace store tests

use super::store::WorkspaceStore;
use super::types::{NewWorkspace, ScopeMode, WorkspaceStatus, WorkspaceType};
use crate::store::database::Database;
use crate::store::task::{NewTask, TaskStore};

fn create_test_db() -> Database {
    Database::in_memory().expect("Failed to create in-memory database")
}

/// Helper to create a parent task for FK constraint
fn create_parent_task(db: &Database, task_id: &str) {
    let task_store = TaskStore::new(db);
    task_store
        .create(NewTask {
            id: Some(task_id.to_string()),
            title: "Parent task".to_string(),
            description: Some("Parent task for test".to_string()),
            ..Default::default()
        })
        .expect("Failed to create parent task");
}

#[test]
fn test_create_workspace() {
    let db = create_test_db();
    create_parent_task(&db, "task-001");
    let store = WorkspaceStore::new(&db);

    let workspace = store
        .create(NewWorkspace {
            id: Some("ws-001".to_string()),
            task_id: "task-001".to_string(),
            run_id: None,
            workspace_type: WorkspaceType::Direct,
            path: "/tmp/workspace".to_string(),
            branch_name: None,
            base_ref: None,
            repo_root: Some("/tmp/repo".to_string()),
            task_scope_path: None,
            scope_mode: None,
            metadata: None,
        })
        .expect("Failed to create workspace");

    assert_eq!(workspace.id, "ws-001");
    assert_eq!(workspace.task_id, "task-001");
    assert_eq!(workspace.workspace_type, WorkspaceType::Direct);
    assert_eq!(workspace.status, WorkspaceStatus::Creating);
}

#[test]
fn test_update_workspace_status() {
    let db = create_test_db();
    create_parent_task(&db, "task-002");
    let store = WorkspaceStore::new(&db);

    store
        .create(NewWorkspace {
            id: Some("ws-002".to_string()),
            task_id: "task-002".to_string(),
            run_id: None,
            workspace_type: WorkspaceType::Worktree,
            path: "/tmp/worktree".to_string(),
            branch_name: Some("feature-branch".to_string()),
            base_ref: Some("main".to_string()),
            repo_root: None,
            task_scope_path: None,
            scope_mode: None,
            metadata: None,
        })
        .expect("Failed to create workspace");

    store
        .update_status("ws-002", WorkspaceStatus::Ready)
        .expect("Failed to update status");

    let workspace = store
        .get("ws-002")
        .expect("Failed to get workspace")
        .expect("Workspace not found");
    assert_eq!(workspace.status, WorkspaceStatus::Ready);
    assert!(workspace.cleaned_at.is_none());
}

#[test]
fn test_cleanup_workspace() {
    let db = create_test_db();
    create_parent_task(&db, "task-003");
    let store = WorkspaceStore::new(&db);

    store
        .create(NewWorkspace {
            id: Some("ws-003".to_string()),
            task_id: "task-003".to_string(),
            run_id: None,
            workspace_type: WorkspaceType::Worktree,
            path: "/tmp/worktree-cleanup".to_string(),
            branch_name: None,
            base_ref: None,
            repo_root: None,
            task_scope_path: None,
            scope_mode: None,
            metadata: None,
        })
        .expect("Failed to create workspace");

    store
        .cleanup_workspace("ws-003")
        .expect("Failed to cleanup workspace");

    let workspace = store
        .get("ws-003")
        .expect("Failed to get workspace")
        .expect("Workspace not found");
    assert_eq!(workspace.status, WorkspaceStatus::Cleaned);
    assert!(workspace.cleaned_at.is_some());
}

#[test]
fn test_get_active_workspaces() {
    let db = create_test_db();
    create_parent_task(&db, "task-active-001");
    let store = WorkspaceStore::new(&db);

    // Create workspaces with different statuses
    store
        .create(NewWorkspace {
            id: Some("ws-active-1".to_string()),
            task_id: "task-active-001".to_string(),
            run_id: None,
            workspace_type: WorkspaceType::Direct,
            path: "/tmp/ws1".to_string(),
            branch_name: None,
            base_ref: None,
            repo_root: None,
            task_scope_path: None,
            scope_mode: None,
            metadata: None,
        })
        .expect("Failed to create workspace");

    store
        .update_status("ws-active-1", WorkspaceStatus::Ready)
        .expect("Failed to update");

    let active = store
        .get_active_workspaces()
        .expect("Failed to get active workspaces");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "ws-active-1");
}

#[test]
fn test_scope_mode() {
    let db = create_test_db();
    create_parent_task(&db, "task-scope-001");
    let store = WorkspaceStore::new(&db);

    let workspace = store
        .create(NewWorkspace {
            id: Some("ws-scope".to_string()),
            task_id: "task-scope-001".to_string(),
            run_id: None,
            workspace_type: WorkspaceType::Direct,
            path: "/tmp/ws".to_string(),
            branch_name: None,
            base_ref: None,
            repo_root: Some("/repo".to_string()),
            task_scope_path: Some("/repo/src".to_string()),
            scope_mode: Some(ScopeMode::Subdir),
            metadata: None,
        })
        .expect("Failed to create workspace");

    assert_eq!(workspace.scope_mode, ScopeMode::Subdir);
    assert_eq!(workspace.task_scope_path, Some("/repo/src".to_string()));
}
