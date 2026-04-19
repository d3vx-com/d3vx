//! Shared test helpers for the coordination agent-tools tests.
//!
//! Kept in a dedicated helpers file so the sibling test files can stay
//! focused on assertions and fit under the 300-line guideline.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::{Tool, ToolContext, ToolResult};

/// Create a unique temp directory under the OS temp dir.
pub(super) fn tmp_root(prefix: &str) -> PathBuf {
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

/// Build a `ToolContext` whose `session_id` is the given agent id.
/// The coordination tools resolve the caller's agent id from this field.
pub(super) fn ctx_for(agent_id: &str) -> ToolContext {
    ToolContext {
        session_id: Some(agent_id.to_string()),
        ..ToolContext::default()
    }
}

/// Look up a tool in a toolset by name; panics if not found so tests
/// give a clear message instead of None-unwrap surprises.
pub(super) fn find_tool<'a>(
    tools: &'a [Arc<dyn Tool>],
    name: &str,
) -> &'a Arc<dyn Tool> {
    tools
        .iter()
        .find(|t| t.definition().name == name)
        .unwrap_or_else(|| panic!("no tool named `{name}`"))
}

/// Invoke a tool with the given input and context.
pub(super) async fn call(
    tool: &Arc<dyn Tool>,
    input: serde_json::Value,
    ctx: &ToolContext,
) -> ToolResult {
    tool.execute(input, ctx).await
}
