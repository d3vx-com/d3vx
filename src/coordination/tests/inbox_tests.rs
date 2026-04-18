//! Tests for per-agent inboxes and the broadcast log.

use std::fs;
use std::path::PathBuf;

use crate::coordination::inbox::{BroadcastLog, Inbox, Message};

fn tmp_dir(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-coord-inbox-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn msg(from: &str, to: &str, body: &str) -> Message {
    Message::new(from, to, body)
}

#[test]
fn open_creates_inbox_file_lazily_on_send() {
    let dir = tmp_dir("lazy");
    let inbox = Inbox::open(&dir, "alpha").unwrap();
    // Opening alone doesn't create the inbox file.
    assert!(!inbox.path().exists());
    inbox.send(&msg("beta", "alpha", "hi")).unwrap();
    assert!(inbox.path().exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn send_then_read_all_preserves_order() {
    let dir = tmp_dir("order");
    let inbox = Inbox::open(&dir, "alpha").unwrap();
    for i in 0..5 {
        inbox.send(&msg("beta", "alpha", &format!("m{i}"))).unwrap();
    }
    let all = inbox.read_all().unwrap();
    let bodies: Vec<_> = all.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, vec!["m0", "m1", "m2", "m3", "m4"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn drain_returns_messages_and_empties_inbox() {
    let dir = tmp_dir("drain");
    let inbox = Inbox::open(&dir, "alpha").unwrap();
    inbox.send(&msg("beta", "alpha", "first")).unwrap();
    inbox.send(&msg("gamma", "alpha", "second")).unwrap();

    let drained = inbox.drain().unwrap();
    assert_eq!(drained.len(), 2);
    assert!(inbox.read_all().unwrap().is_empty());
    assert!(inbox.is_empty().unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn len_and_is_empty_are_consistent() {
    let dir = tmp_dir("len");
    let inbox = Inbox::open(&dir, "alpha").unwrap();
    assert_eq!(inbox.len().unwrap(), 0);
    assert!(inbox.is_empty().unwrap());
    inbox.send(&msg("beta", "alpha", "hi")).unwrap();
    assert_eq!(inbox.len().unwrap(), 1);
    assert!(!inbox.is_empty().unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn two_inboxes_same_dir_do_not_interfere() {
    let dir = tmp_dir("two");
    let alpha = Inbox::open(&dir, "alpha").unwrap();
    let beta = Inbox::open(&dir, "beta").unwrap();

    alpha.send(&msg("x", "alpha", "for alpha")).unwrap();
    beta.send(&msg("x", "beta", "for beta")).unwrap();
    beta.send(&msg("x", "beta", "also for beta")).unwrap();

    assert_eq!(alpha.read_all().unwrap().len(), 1);
    assert_eq!(beta.read_all().unwrap().len(), 2);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inbox_agent_id_round_trips() {
    let dir = tmp_dir("id");
    let inbox = Inbox::open(&dir, "coordinator").unwrap();
    assert_eq!(inbox.agent_id(), "coordinator");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn send_from_many_writers_serializes_correctly() {
    // Threads append concurrently to the same inbox; every message
    // must survive as a parseable line.
    let dir = tmp_dir("mt_send");
    let inbox = Inbox::open(&dir, "alpha").unwrap();
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let ib = inbox.clone();
            std::thread::spawn(move || {
                for j in 0..5 {
                    ib.send(&msg(&format!("w{i}"), "alpha", &format!("m{i}-{j}")))
                        .unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let all = inbox.read_all().unwrap();
    assert_eq!(all.len(), 8 * 5);
    fs::remove_dir_all(&dir).unwrap();
}

// ── BroadcastLog ────────────────────────────────────────────────

#[test]
fn broadcast_publish_and_read_all_preserves_order() {
    let dir = tmp_dir("bc_order");
    let log = BroadcastLog::open(dir.join("bc.jsonl")).unwrap();
    for i in 0..3 {
        log.publish(&msg("coord", "*", &format!("b{i}"))).unwrap();
    }
    let all = log.read_all().unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].body, "b2");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn broadcast_read_since_respects_offset() {
    let dir = tmp_dir("bc_since");
    let log = BroadcastLog::open(dir.join("bc.jsonl")).unwrap();
    log.publish(&msg("c", "*", "a")).unwrap();
    log.publish(&msg("c", "*", "b")).unwrap();
    log.publish(&msg("c", "*", "c")).unwrap();

    let from_1 = log.read_since(1).unwrap();
    let bodies: Vec<_> = from_1.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, vec!["b", "c"]);

    // Offset past the end returns empty, not an error.
    assert!(log.read_since(99).unwrap().is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn broadcast_len_tracks_growth() {
    let dir = tmp_dir("bc_len");
    let log = BroadcastLog::open(dir.join("bc.jsonl")).unwrap();
    assert_eq!(log.len().unwrap(), 0);
    assert!(log.is_empty().unwrap());
    log.publish(&msg("c", "*", "x")).unwrap();
    assert_eq!(log.len().unwrap(), 1);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn broadcast_creates_parent_dir_if_missing() {
    let dir = tmp_dir("bc_parent");
    let nested = dir.join("nested/deeper/log.jsonl");
    let log = BroadcastLog::open(&nested).unwrap();
    log.publish(&msg("c", "*", "x")).unwrap();
    assert!(nested.exists());
    fs::remove_dir_all(&dir).unwrap();
}
