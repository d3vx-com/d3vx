use super::{FileChangeLog, FileSnapshot};
use std::fs;

#[test]
fn test_empty_log() {
    let log = FileChangeLog::new();
    assert!(log.is_empty());
    assert!(log.files_after(0).is_empty());
}

#[test]
fn test_snapshot_for_write_new_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    let mut log = FileChangeLog::new();
    let calls = vec![(
        "t1".to_string(),
        "Write".to_string(),
        serde_json::json!({"file_path": file_path_str, "content": "hello"}),
    )];

    log.snapshot_for(5, &calls, &temp_dir.path().to_string_lossy());

    assert_eq!(log.len(), 1);
    let (_, snapshots) = &log.entries[0];
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].file_path, file_path_str);
    assert!(snapshots[0].old_content.is_none());
}

#[test]
fn test_snapshot_for_write_existing_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("existing.txt");
    fs::write(&file_path, "old content").unwrap();

    let mut log = FileChangeLog::new();
    let calls = vec![(
        "t1".to_string(),
        "Write".to_string(),
        serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "content": "new content"
        }),
    )];

    log.snapshot_for(2, &calls, &temp_dir.path().to_string_lossy());

    let (_, snapshots) = &log.entries[0];
    assert_eq!(snapshots[0].old_content.as_deref(), Some("old content"));
}

#[test]
fn test_snapshot_for_edit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("edit_me.txt");
    fs::write(&file_path, "original").unwrap();

    let mut log = FileChangeLog::new();
    let calls = vec![(
        "t1".to_string(),
        "Edit".to_string(),
        serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "old_string": "original",
            "new_string": "modified"
        }),
    )];

    log.snapshot_for(3, &calls, &temp_dir.path().to_string_lossy());

    let (_, snapshots) = &log.entries[0];
    assert_eq!(snapshots[0].old_content.as_deref(), Some("original"));
}

#[test]
fn test_snapshot_ignores_non_file_tools() {
    let mut log = FileChangeLog::new();
    let calls = vec![(
        "t1".to_string(),
        "Read".to_string(),
        serde_json::json!({"file_path": "/some/file.txt"}),
    )];

    log.snapshot_for(1, &calls, ".");
    assert!(log.is_empty());
}

#[test]
fn test_revert_to_restores_content() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "old content").unwrap();

    let mut log = FileChangeLog::new();
    log.entries.push((
        3,
        vec![FileSnapshot {
            file_path: file_path.to_string_lossy().to_string(),
            old_content: Some("old content".to_string()),
        }],
    ));

    fs::write(&file_path, "new content").unwrap();

    let reverted = log.revert_to(2);

    assert_eq!(reverted.len(), 1);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "old content");
}

#[test]
fn test_revert_to_deletes_new_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");

    let mut log = FileChangeLog::new();
    log.entries.push((
        1,
        vec![FileSnapshot {
            file_path: file_path.to_string_lossy().to_string(),
            old_content: None,
        }],
    ));

    fs::write(&file_path, "new content").unwrap();
    assert!(file_path.exists());

    let reverted = log.revert_to(0);

    assert_eq!(reverted.len(), 1);
    assert!(!file_path.exists());
}

#[test]
fn test_revert_uses_earliest_snapshot() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("multi.txt");

    let mut log = FileChangeLog::new();
    log.entries.push((
        2,
        vec![FileSnapshot {
            file_path: file_path.to_string_lossy().to_string(),
            old_content: Some("version 1".to_string()),
        }],
    ));
    log.entries.push((
        4,
        vec![FileSnapshot {
            file_path: file_path.to_string_lossy().to_string(),
            old_content: Some("version 2".to_string()),
        }],
    ));

    let reverted = log.revert_to(1);

    assert_eq!(reverted.len(), 1);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "version 1");
}

#[test]
fn test_files_after() {
    let mut log = FileChangeLog::new();
    log.entries.push((
        2,
        vec![FileSnapshot {
            file_path: "/a.txt".to_string(),
            old_content: None,
        }],
    ));
    log.entries.push((
        4,
        vec![
            FileSnapshot {
                file_path: "/b.txt".to_string(),
                old_content: None,
            },
            FileSnapshot {
                file_path: "/a.txt".to_string(),
                old_content: Some("content".to_string()),
            },
        ],
    ));

    let files = log.files_after(1);
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"/a.txt".to_string()));
    assert!(files.contains(&"/b.txt".to_string()));

    assert_eq!(log.files_after(3).len(), 2);
    assert!(log.files_after(5).is_empty());
}

#[test]
fn test_truncate() {
    let mut log = FileChangeLog::new();
    log.entries.push((
        1,
        vec![FileSnapshot {
            file_path: "/a.txt".to_string(),
            old_content: None,
        }],
    ));
    log.entries.push((
        3,
        vec![FileSnapshot {
            file_path: "/b.txt".to_string(),
            old_content: None,
        }],
    ));
    log.entries.push((
        5,
        vec![FileSnapshot {
            file_path: "/c.txt".to_string(),
            old_content: None,
        }],
    ));

    log.truncate(3);

    assert_eq!(log.len(), 2);
    assert!(log.entries.iter().all(|(idx, _)| *idx <= 3));
}

#[test]
fn test_resolve_path_relative() {
    let resolved = FileChangeLog::resolve_path("src/main.rs", "/home/user/project");
    assert_eq!(resolved, "/home/user/project/src/main.rs");
}

#[test]
fn test_resolve_path_absolute() {
    let resolved = FileChangeLog::resolve_path("/tmp/test.txt", "/home/user/project");
    assert_eq!(resolved, "/tmp/test.txt");
}
