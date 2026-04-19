//! Per-operation tests for the five coordination tools.
//!
//! Factory/schema/preamble tests live in `agent_tools_tests.rs`; this
//! file exercises each tool's runtime behaviour: claiming, completing,
//! sending messages, draining inboxes, and the invariants across
//! agents sharing one toolset.

use std::fs;

use crate::coordination::agent_tools::CoordinationToolset;
use crate::coordination::board::{CoordinationBoard, NewTask};
use crate::coordination::inbox::{Inbox, Message};
use crate::coordination::tests::agent_tools_helpers::{call, ctx_for, find_tool, tmp_root};
use crate::tools::ToolContext;

#[tokio::test]
async fn list_ready_tasks_returns_only_unclaimed_tasks_with_resolved_deps() {
    let root = tmp_root("ready");
    let ts = CoordinationToolset::new(&root).unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("a", "A", "do a")).unwrap();
    board
        .add_task(NewTask::new("b", "B", "do b").with_depends_on(vec!["a".to_string()]))
        .unwrap();

    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_list_ready_tasks");
    let out = call(tool, serde_json::json!({}), &ctx_for("alpha")).await;
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
async fn claim_task_takes_owner_from_tool_context_session_id() {
    let root = tmp_root("claim_ctx");
    let ts = CoordinationToolset::new(&root).unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();

    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_claim_task");
    let out = call(
        tool,
        serde_json::json!({ "task_id": "t" }),
        &ctx_for("alpha"),
    )
    .await;
    assert!(!out.is_error, "claim should succeed: {}", out.content);

    let loaded = board.get_task("t").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("alpha"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn claim_task_errors_when_context_lacks_session_id() {
    let root = tmp_root("claim_nosid");
    let ts = CoordinationToolset::new(&root).unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();

    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_claim_task");
    let mut ctx = ToolContext::default();
    ctx.session_id = None;
    let out = call(tool, serde_json::json!({ "task_id": "t" }), &ctx).await;
    assert!(out.is_error);
    assert!(out.content.contains("session_id"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn two_agents_sharing_toolset_see_each_others_tasks_but_not_claims() {
    let root = tmp_root("share");
    let ts = CoordinationToolset::new(&root).unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();

    let tools = ts.tools();
    let claim = find_tool(&tools, "coord_claim_task");
    // Alpha claims.
    assert!(!call(claim, serde_json::json!({ "task_id": "t" }), &ctx_for("alpha"))
        .await
        .is_error);
    // Beta (sharing the same toolset) must see the task already claimed.
    let beta_out = call(
        claim,
        serde_json::json!({ "task_id": "t" }),
        &ctx_for("beta"),
    )
    .await;
    assert!(beta_out.is_error, "second claim must fail");
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn claim_task_errors_on_missing_argument() {
    let root = tmp_root("claim_missing");
    let ts = CoordinationToolset::new(&root).unwrap();
    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_claim_task");
    let out = call(tool, serde_json::json!({}), &ctx_for("alpha")).await;
    assert!(out.is_error);
    assert!(out.content.contains("task_id"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn complete_task_moves_task_to_completed_with_result() {
    let root = tmp_root("complete");
    let ts = CoordinationToolset::new(&root).unwrap();
    let board = CoordinationBoard::open(root.join("tasks")).unwrap();
    board.add_task(NewTask::new("t", "T", "do")).unwrap();
    board.claim_task("t", "alpha").unwrap();

    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_complete_task");
    let out = call(
        tool,
        serde_json::json!({ "task_id": "t", "result": "ok" }),
        &ctx_for("alpha"),
    )
    .await;
    assert!(!out.is_error, "complete: {}", out.content);

    let loaded = board.get_task("t").unwrap().unwrap();
    assert_eq!(loaded.status, crate::coordination::TaskStatus::Completed);
    assert_eq!(loaded.result.as_deref(), Some("ok"));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn send_message_uses_context_session_id_as_from() {
    let root = tmp_root("send");
    let ts = CoordinationToolset::new(&root).unwrap();
    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_send_message");
    let out = call(
        tool,
        serde_json::json!({ "to": "beta", "body": "hello" }),
        &ctx_for("alpha"),
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
async fn drain_inbox_resolves_inbox_from_context_session_id() {
    let root = tmp_root("drain_ctx");
    let ts = CoordinationToolset::new(&root).unwrap();
    // Seed inboxes for two distinct agents.
    let alpha_inbox = Inbox::open(root.join("inboxes"), "alpha").unwrap();
    alpha_inbox.send(&Message::new("beta", "alpha", "for alpha")).unwrap();
    let beta_inbox = Inbox::open(root.join("inboxes"), "beta").unwrap();
    beta_inbox.send(&Message::new("alpha", "beta", "for beta")).unwrap();

    let tools = ts.tools();
    let drain = find_tool(&tools, "coord_drain_inbox");

    // Alpha drains its own inbox only.
    let out = call(drain, serde_json::json!({}), &ctx_for("alpha")).await;
    assert!(!out.is_error);
    let arr: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    let bodies: Vec<&str> = arr
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["body"].as_str().unwrap())
        .collect();
    assert_eq!(bodies, vec!["for alpha"]);

    // Beta's inbox is unaffected.
    assert_eq!(beta_inbox.read_all().unwrap().len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn drain_inbox_is_safe_when_empty() {
    let root = tmp_root("drain_empty");
    let ts = CoordinationToolset::new(&root).unwrap();
    let tools = ts.tools();
    let tool = find_tool(&tools, "coord_drain_inbox");
    let out = call(tool, serde_json::json!({}), &ctx_for("alpha")).await;
    assert!(!out.is_error);
    let arr: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert!(arr.as_array().unwrap().is_empty());
    fs::remove_dir_all(&root).ok();
}
