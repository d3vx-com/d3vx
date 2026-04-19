//! Factory, schema, registration, and preamble tests for the
//! coordination agent-tools layer.
//!
//! Per-tool-operation behaviour lives in `agent_tools_ops_tests.rs`;
//! this file exercises the shape of the toolset itself.

use std::fs;

use crate::coordination::agent_tools::CoordinationToolset;
use crate::coordination::prompt::coordination_preamble;
use crate::coordination::tests::agent_tools_helpers::tmp_root;

#[test]
fn toolset_new_creates_tasks_and_inboxes_dirs() {
    let root = tmp_root("dirs");
    let _ts = CoordinationToolset::new(&root).unwrap();
    assert!(root.join("tasks").is_dir());
    assert!(root.join("inboxes").is_dir());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toolset_exposes_five_named_tools() {
    let root = tmp_root("names");
    let ts = CoordinationToolset::new(&root).unwrap();
    let names: Vec<String> = ts.tools().iter().map(|t| t.definition().name).collect();
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
    let ts = CoordinationToolset::new(&root).unwrap();
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
async fn register_on_puts_every_coordination_tool_on_the_coordinator() {
    use crate::agent::ToolCoordinator;
    let root = tmp_root("register");
    let ts = CoordinationToolset::new(&root).unwrap();
    let coord = ToolCoordinator::new();
    ts.register_on(&coord).await;
    for name in [
        "coord_list_ready_tasks",
        "coord_claim_task",
        "coord_complete_task",
        "coord_send_message",
        "coord_drain_inbox",
    ] {
        assert!(coord.has_tool(name).await, "missing {name}");
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn preamble_mentions_every_tool_name() {
    let p = coordination_preamble();
    for name in [
        "coord_list_ready_tasks",
        "coord_claim_task",
        "coord_complete_task",
        "coord_send_message",
        "coord_drain_inbox",
    ] {
        assert!(p.contains(name), "preamble missing tool name `{name}`");
    }
}

#[test]
fn preamble_is_reasonably_short() {
    // Long preambles compete with the task prompt for attention.
    // 2 KB is a soft ceiling — if we blow past it, re-think scope.
    let p = coordination_preamble();
    assert!(p.len() < 2048, "preamble too long: {} bytes", p.len());
}

#[tokio::test]
async fn attach_to_agent_registers_tools_and_replaces_empty_system_prompt() {
    use crate::agent::{AgentConfig, ToolCoordinator};
    let root = tmp_root("attach_empty");
    let ts = CoordinationToolset::new(&root).unwrap();
    let coord = ToolCoordinator::new();
    let mut cfg = AgentConfig {
        system_prompt: String::new(),
        ..AgentConfig::default()
    };

    ts.attach_to_agent(&coord, &mut cfg).await;

    assert!(coord.has_tool("coord_claim_task").await);
    assert!(coord.has_tool("coord_send_message").await);
    assert_eq!(cfg.system_prompt, coordination_preamble());
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn attach_to_agent_prepends_preamble_to_existing_system_prompt() {
    use crate::agent::{AgentConfig, ToolCoordinator};
    let root = tmp_root("attach_prepend");
    let ts = CoordinationToolset::new(&root).unwrap();
    let coord = ToolCoordinator::new();
    let original = "You are a careful reviewer.";
    let mut cfg = AgentConfig {
        system_prompt: original.to_string(),
        ..AgentConfig::default()
    };

    ts.attach_to_agent(&coord, &mut cfg).await;

    let preamble = coordination_preamble();
    assert!(
        cfg.system_prompt.starts_with(&preamble),
        "preamble must come first"
    );
    assert!(
        cfg.system_prompt.ends_with(original),
        "original prompt must be preserved"
    );
    assert!(coord.has_tool("coord_drain_inbox").await);
    fs::remove_dir_all(&root).ok();
}
