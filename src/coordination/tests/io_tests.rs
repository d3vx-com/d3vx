//! Tests for atomic JSON / JSONL helpers.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::coordination::io;

fn tmp_dir(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-coord-io-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Sample {
    id: u32,
    name: String,
}

#[test]
fn atomic_write_json_creates_and_overwrites() {
    let dir = tmp_dir("write");
    let path = dir.join("a.json");

    io::atomic_write_json(&path, &Sample { id: 1, name: "one".into() }).unwrap();
    assert!(path.exists());

    io::atomic_write_json(&path, &Sample { id: 2, name: "two".into() }).unwrap();
    let re: Sample = io::read_json_if_exists(&path).unwrap().unwrap();
    assert_eq!(re, Sample { id: 2, name: "two".into() });

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn atomic_write_leaves_no_tempfile_on_success() {
    let dir = tmp_dir("tmp_clean");
    let path = dir.join("b.json");
    io::atomic_write_json(&path, &Sample { id: 9, name: "x".into() }).unwrap();

    // Nothing should remain in the directory other than the target file.
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name(), "b.json");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_json_if_exists_returns_none_for_missing_file() {
    let dir = tmp_dir("missing");
    let value: Option<Sample> = io::read_json_if_exists(dir.join("nope.json")).unwrap();
    assert!(value.is_none());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_json_surfaces_parse_errors_with_path() {
    let dir = tmp_dir("bad_json");
    let path = dir.join("corrupt.json");
    fs::write(&path, "{ not json").unwrap();
    let res: Result<Option<Sample>, _> = io::read_json_if_exists(&path);
    assert!(res.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn create_exclusive_returns_true_then_false_on_racers() {
    let dir = tmp_dir("excl");
    let path = dir.join("claim");

    let first = io::create_exclusive(&path, b"alpha").unwrap();
    let second = io::create_exclusive(&path, b"beta").unwrap();
    assert!(first, "first caller should win");
    assert!(!second, "second caller should see existing");
    assert_eq!(fs::read_to_string(&path).unwrap(), "alpha");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn append_and_read_jsonl_round_trips() {
    let dir = tmp_dir("jsonl");
    let path = dir.join("log.jsonl");

    for i in 0..3 {
        io::append_jsonl(&path, &Sample { id: i, name: format!("n{i}") }).unwrap();
    }
    let rows: Vec<Sample> = io::read_jsonl(&path).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, 0);
    assert_eq!(rows[2].name, "n2");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_jsonl_empty_for_missing_file() {
    let dir = tmp_dir("jsonl_missing");
    let rows: Vec<Sample> = io::read_jsonl(dir.join("nope.jsonl")).unwrap();
    assert!(rows.is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_jsonl_skips_blank_lines_and_fails_on_corruption() {
    let dir = tmp_dir("jsonl_mixed");
    let path = dir.join("mixed.jsonl");
    // Two valid rows, a blank, then one corrupt — should error on corrupt.
    fs::write(
        &path,
        "{\"id\":1,\"name\":\"a\"}\n\n{\"id\":2,\"name\":\"b\"}\nnot json\n",
    )
    .unwrap();
    let res: Result<Vec<Sample>, _> = io::read_jsonl(&path);
    assert!(res.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn truncate_file_empties_existing_content() {
    let dir = tmp_dir("trunc");
    let path = dir.join("to_truncate");
    fs::write(&path, "old contents").unwrap();
    io::truncate_file(&path).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn truncate_file_creates_missing() {
    let dir = tmp_dir("trunc_new");
    let path = dir.join("brand_new");
    assert!(!path.exists());
    io::truncate_file(&path).unwrap();
    assert!(path.exists());
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
    fs::remove_dir_all(&dir).unwrap();
}
