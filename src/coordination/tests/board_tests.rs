//! Tests for the coordination task board.

use std::fs;
use std::path::PathBuf;

use crate::coordination::board::{CoordinationBoard, NewTask, TaskStatus};
use crate::coordination::errors::CoordinationError;

fn tmp_board(prefix: &str) -> (CoordinationBoard, PathBuf) {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-coord-board-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let board = CoordinationBoard::open(&p).unwrap();
    (board, p)
}

fn task(id: &str) -> NewTask {
    NewTask::new(id, format!("Title {id}"), format!("Do {id}"))
}

#[test]
fn open_creates_tasks_directory() {
    let (board, root) = tmp_board("open");
    assert!(root.exists());
    assert_eq!(board.root(), root);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn add_task_persists_as_pending_with_no_owner() {
    let (board, root) = tmp_board("add");
    let t = board.add_task(task("a")).unwrap();
    assert_eq!(t.id, "a");
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.owner.is_none());

    // Round-trip through disk.
    let reloaded = board.get_task("a").unwrap().unwrap();
    assert_eq!(reloaded, t);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn add_task_rejects_duplicate_id() {
    let (board, root) = tmp_board("dup");
    board.add_task(task("a")).unwrap();
    let err = board.add_task(task("a")).unwrap_err();
    match err {
        CoordinationError::Io { source, .. } => {
            assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists);
        }
        other => panic!("expected Io AlreadyExists, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn get_task_returns_none_for_unknown() {
    let (board, root) = tmp_board("unknown");
    assert!(board.get_task("ghost").unwrap().is_none());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn list_tasks_is_ordered_by_creation() {
    let (board, root) = tmp_board("order");
    board.add_task(task("first")).unwrap();
    // Tiny gap so timestamps differ.
    std::thread::sleep(std::time::Duration::from_millis(2));
    board.add_task(task("second")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    board.add_task(task("third")).unwrap();

    let tasks = board.list_tasks().unwrap();
    let ids: Vec<_> = tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["first", "second", "third"]);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn claim_task_transitions_pending_to_claimed_with_owner() {
    let (board, root) = tmp_board("claim_ok");
    board.add_task(task("a")).unwrap();
    let claimed = board.claim_task("a", "alpha").unwrap();
    assert_eq!(claimed.status, TaskStatus::Claimed);
    assert_eq!(claimed.owner.as_deref(), Some("alpha"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn claim_task_rejects_already_claimed_with_current_owner() {
    let (board, root) = tmp_board("claim_race");
    board.add_task(task("a")).unwrap();
    board.claim_task("a", "alpha").unwrap();
    let err = board.claim_task("a", "beta").unwrap_err();
    match err {
        CoordinationError::InvalidTransition { .. } => {
            // Task status is already Claimed, so invalid transition fires
            // before the atomic claim file is consulted.
        }
        other => panic!("expected InvalidTransition, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn claim_task_returns_not_found_for_unknown_task() {
    let (board, root) = tmp_board("claim_unknown");
    let err = board.claim_task("ghost", "alpha").unwrap_err();
    assert!(matches!(err, CoordinationError::TaskNotFound { .. }));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn claim_task_refuses_when_deps_not_complete() {
    let (board, root) = tmp_board("claim_deps");
    board.add_task(task("a")).unwrap();
    board
        .add_task(NewTask::new("b", "B", "B").with_depends_on(vec!["a".to_string()]))
        .unwrap();
    let err = board.claim_task("b", "alpha").unwrap_err();
    match err {
        CoordinationError::NotReady { unresolved, .. } => {
            assert_eq!(unresolved, vec!["a".to_string()]);
        }
        other => panic!("expected NotReady, got {other:?}"),
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn complete_task_marks_completed_and_stores_result() {
    let (board, root) = tmp_board("complete");
    board.add_task(task("a")).unwrap();
    board.claim_task("a", "alpha").unwrap();
    let done = board.complete_task("a", "final output").unwrap();
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(done.result.as_deref(), Some("final output"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn complete_releases_claim_file_for_clean_state() {
    let (board, root) = tmp_board("release");
    board.add_task(task("a")).unwrap();
    board.claim_task("a", "alpha").unwrap();
    let claim_path = root.join("a.claim");
    assert!(claim_path.exists());
    board.complete_task("a", "ok").unwrap();
    assert!(!claim_path.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn fail_task_sets_failed_with_reason_as_result() {
    let (board, root) = tmp_board("fail");
    board.add_task(task("a")).unwrap();
    board.claim_task("a", "alpha").unwrap();
    let t = board.fail_task("a", "blew up").unwrap();
    assert_eq!(t.status, TaskStatus::Failed);
    assert_eq!(t.result.as_deref(), Some("blew up"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cancel_task_works_from_pending() {
    let (board, root) = tmp_board("cancel_pending");
    board.add_task(task("a")).unwrap();
    let t = board.cancel_task("a").unwrap();
    assert_eq!(t.status, TaskStatus::Cancelled);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cannot_re_complete_terminal_task() {
    let (board, root) = tmp_board("reterm");
    board.add_task(task("a")).unwrap();
    board.claim_task("a", "alpha").unwrap();
    board.complete_task("a", "ok").unwrap();
    let err = board.complete_task("a", "again").unwrap_err();
    assert!(matches!(err, CoordinationError::InvalidTransition { .. }));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn list_ready_tasks_respects_dep_graph() {
    let (board, root) = tmp_board("ready");
    board.add_task(task("a")).unwrap();
    board
        .add_task(NewTask::new("b", "B", "B").with_depends_on(vec!["a".to_string()]))
        .unwrap();
    board
        .add_task(NewTask::new("c", "C", "C").with_depends_on(vec!["a".to_string()]))
        .unwrap();

    // Only `a` is ready initially.
    let ready: Vec<_> =
        board.list_ready_tasks().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(ready, vec!["a".to_string()]);

    board.claim_task("a", "alpha").unwrap();
    // Still only `a` counted — it's claimed, not completed.
    assert_eq!(board.list_ready_tasks().unwrap().len(), 0);

    board.complete_task("a", "done").unwrap();
    let ready: Vec<_> = board
        .list_ready_tasks()
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert!(ready.contains(&"b".to_string()));
    assert!(ready.contains(&"c".to_string()));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn claim_is_atomic_under_concurrent_callers() {
    // Two threads race on the same task. Exactly one should succeed;
    // the other must see AlreadyClaimed.
    let (board, root) = tmp_board("race");
    board.add_task(task("a")).unwrap();

    let b1 = board.clone();
    let b2 = board.clone();
    let h1 = std::thread::spawn(move || b1.claim_task("a", "alpha"));
    let h2 = std::thread::spawn(move || b2.claim_task("a", "beta"));
    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();

    let (ok_count, err_count) = match (&r1, &r2) {
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => (1, 1),
        (Ok(_), Ok(_)) => (2, 0),
        (Err(_), Err(_)) => (0, 2),
    };
    assert_eq!(
        (ok_count, err_count),
        (1, 1),
        "exactly one thread must win the claim race"
    );
    fs::remove_dir_all(&root).unwrap();
}
