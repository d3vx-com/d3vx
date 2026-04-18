//! Tests for the coordination agent tools and prompt preamble.

use std::fs;
use std::path::PathBuf;

use crate::coordination::agent_tools::CoordinationToolset;
use crate::coordination::board::{CoordinationBoard, NewTask};
use crate::coordination::inbox::{Inbox, Message};
use crate::coordination::prompt::coordination_preamble;
use crate::tools::{ToolContext, ToolResult};

fn tmp_root(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-coord-tools-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn find_tool<'a>(
    tools: &'a [std::sync::Arc<dyn crate::tools::Tool>],
    name: &str,
) -> &'a std::sync::Arc<dyn crate::tools::Tool> {
    tools
        .iter()
        .find(|t| t.definition().name == name)
        .unwrap_or_else(|| panic!("no tool named `{name}`"))
}

async fn call(
    tool: &std::sync::Arc<dyn crate::tools::Tool>,
    input: serde_json::Value,
) -> ToolResult {
    tool.execute(input, &ToolContext::default()).await
}

#[test]
fn toolset_new_creates_tasks_and_inboxes_dirs() {
    let root = tmp_root("dirs");
    let _ts = CoordinationToolset::new(&root, "alpha").unwrap();
    assert!(root.join("tasks").is_dir());
    assert!(root.join("inboxes").is_dir());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toolset_exposes_five_named_tools() {
    let root = tmp_root("names");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let names: Vec<String> = ts
        .tools()
        .iter()
        .map(|t| t.definition().name)
        .collect();
    for expected in [
        "coord_list_ready_tasks",
        "coord_claim_task",
        "coord_complete_task",
        "coord_send_message",
        "coord_drain_inbox",
    ] {
        assert!(names.iter().any(|n| n == expected), "missing {expected}");
    }
    assert_eq!(names.len(), 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn every_tool_has_non_empty_description_and_object_schema() {
    let root = tmp_root("schemas");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    for tool in ts.tools() {
        let def = tool.definition();
        assert!(!def.description.is_empty(), "{} has empty desc", def.name);
        assert_eq!(
            def.input_schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "{} schema.type must be object",
            def.name
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn list_ready_tasks_returns_only_unclaimed_tasks_with_resolved_deps() {
    let root = tmp_root("ready");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("a", "A", "do a")).unwrap();
    board
        .add_task(NewTask::new("b", "B", "do b").with_depends_on(vec!["a".to_string()]))
        .unwrap();

    let tool = find_tool(&ts.tools(), "coord_list_ready_tasks").clone();
    let out = call(&tool, serde_json::json!({})).await;
    assert!(!out.is_error);
    let tasks: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    let ids: Vec<&str> = tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["a"]);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn claim_task_updates_owner_and_status() {
    let root = tmp_root("claim");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();

    let tool = find_tool(&ts.tools(), "coord_claim_task").clone();
    let out = call(&tool, serde_json::json!({ "task_id": "t" })).await;
    assert!(!out.is_error, "claim should succeed: {}", out.content);

    let loaded = board.get_task("t").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("alpha"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn claim_task_reports_error_when_already_claimed() {
    let root = tmp_root("claim_race");
    let alpha = CoordinationToolset::new(&root, "alpha").unwrap();
    let beta = CoordinationToolset::new(&root, "beta").unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();

    let alpha_tool = find_tool(&alpha.tools(), "coord_claim_task").clone();
    let beta_tool = find_tool(&beta.tools(), "coord_claim_task").clone();
    assert!(!call(&alpha_tool, serde_json::json!({ "task_id": "t" })).await.is_error);
    let result = call(&beta_tool, serde_json::json!({ "task_id": "t" })).await;
    assert!(result.is_error, "second claimer must see error");
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn claim_task_errors_on_missing_argument() {
    let root = tmp_root("claim_missing");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let tool = find_tool(&ts.tools(), "coord_claim_task").clone();
    let out = call(&tool, serde_json::json!({})).await;
    assert!(out.is_error);
    assert!(out.content.contains("task_id"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn complete_task_moves_task_to_completed_with_result() {
    let root = tmp_root("complete");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();
    board.claim_task("t", "alpha").unwrap();

    let tool = find_tool(&ts.tools(), "coord_complete_task").clone();
    let out =
        call(&tool, serde_json::json!({ "task_id": "t", "result": "ok" })).await;
    assert!(!out.is_error, "complete: {}", out.content);

    let loaded = board.get_task("t").unwrap().unwrap();
    assert_eq!(
        loaded.status,
        crate::coordination::TaskStatus::Completed
    );
    assert_eq!(loaded.result.as_deref(), Some("ok"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn send_message_appears_in_recipient_inbox() {
    let root = tmp_root("send");
    let alpha = CoordinationToolset::new(&root, "alpha").unwrap();
    let tool = find_tool(&alpha.tools(), "coord_send_message").clone();
    let out = call(
        &tool,
        serde_json::json!({ "to": "beta", "body": "hello" }),
    )
    .await;
    assert!(!out.is_error);

    let beta_inbox = Inbox::open(root.join("inboxes"), "beta").unwrap();
    let messages = beta_inbox.read_all().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].from, "alpha");
    assert_eq!(messages[0].body, "hello");
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn drain_inbox_returns_and_clears_messages() {
    let root = tmp_root("drain");
    let alpha = CoordinationToolset::new(&root, "alpha").unwrap();
    // Seed alpha's inbox with two messages from beta.
    let alpha_inbox = Inbox::open(root.join("inboxes"), "alpha").unwrap();
    alpha_inbox.send(&Message::new("beta", "alpha", "one")).unwrap();
    alpha_inbox.send(&Message::new("beta", "alpha", "two")).unwrap();

    let tool = find_tool(&alpha.tools(), "coord_drain_inbox").clone();
    let out = call(&tool, serde_json::json!({})).await;
    assert!(!out.is_error);
    let arr: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert_eq!(arr.as_array().map(|a| a.len()), Some(2));
    assert!(alpha_inbox.is_empty().unwrap());
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn drain_inbox_is_safe_when_empty() {
    let root = tmp_root("drain_empty");
    let ts = CoordinationToolset::new(&root, "alpha").unwrap();
    let tool = find_tool(&ts.tools(), "coord_drain_inbox").clone();
    let out = call(&tool, serde_json::json!({})).await;
    assert!(!out.is_error);
    let arr: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert!(arr.as_array().unwrap().is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn preamble_mentions_every_tool_name() {
    let p = coordination_preamble("alpha-01");
    for name in [
        "coord_list_ready_tasks",
        "coord_claim_task",
        "coord_complete_task",
        "coord_send_message",
        "coord_drain_inbox",
    ] {
        assert!(p.contains(name), "preamble missing tool name `{name}`");
    }
    assert!(p.contains("alpha-01"));
}

#[test]
fn preamble_is_reasonably_short() {
    // Long preambles compete with the actual task prompt for attention.
    // 2 KB is a soft ceiling — if we blow past it, re-think scope.
    let p = coordination_preamble("a");
    assert!(p.len() < 2048, "preamble too long: {} bytes", p.len());
}
