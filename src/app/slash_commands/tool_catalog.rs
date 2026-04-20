//! `/tools` — browse the registered tool catalog.
//!
//! d3vx ships 40+ built-in tools (Bash, Read, Write, Edit, Glob,
//! Grep, Skill, MCP, ...) but until now they were invisible from
//! inside the TUI — users only discovered them when the agent
//! happened to call one. That's a product-shaped black hole: a
//! power user has no way to browse the capability surface.
//!
//! This module adds a single slash command that queries the live
//! `ToolCoordinator` (the single source of truth — no static
//! catalog to drift out of sync with what the agent can actually
//! do), groups results into a handful of discoverable buckets, and
//! renders them as a system message.
//!
//! Grouping is intentional. A flat list of 40 names overwhelms;
//! grouping by a rough capability axis (Filesystem / Shell / Git /
//! Web / Memory / MCP / Other) lets a user scan the block that
//! matches their current task. A tool whose name doesn't match any
//! known group falls into "Other" so we never hide a tool just
//! because the prefix table is incomplete.

use anyhow::Result;

use crate::app::App;

/// `/tools` — list every registered tool, grouped by capability.
pub fn handle_tools(app: &mut App, _args: &[&str]) -> Result<()> {
    let coordinator = app.tools.tool_coordinator.clone();
    let names = tokio::task::block_in_place(|| {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(coordinator.list_tool_names())
    });

    if names.is_empty() {
        app.add_system_message(
            "No tools are registered. This usually means the agent isn't initialised yet — try `/doctor` or start a conversation first.",
        );
        return Ok(());
    }

    let grouped = group_tools(&names);
    let mut out = format!("Tools ({} registered):\n", names.len());
    for (group, tools) in &grouped {
        if tools.is_empty() {
            continue;
        }
        out.push_str(&format!("\n  {group}\n"));
        for name in tools {
            out.push_str(&format!("    {name}\n"));
        }
    }
    app.add_system_message(&out);
    Ok(())
}

/// Bucket tool names into rough capability groups. The group name is
/// what users see in `/tools`; ordering is by most-commonly-needed.
///
/// Prefix rules are intentionally lenient: anything that doesn't
/// match a known prefix falls into "Other" rather than being hidden.
/// That protects the catalog against drift when new tools land
/// without this table being updated.
fn group_tools(names: &[String]) -> Vec<(&'static str, Vec<String>)> {
    let mut filesystem: Vec<String> = Vec::new();
    let mut shell: Vec<String> = Vec::new();
    let mut git: Vec<String> = Vec::new();
    let mut web: Vec<String> = Vec::new();
    let mut memory: Vec<String> = Vec::new();
    let mut mcp: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();

    for raw in names {
        let name = raw.clone();
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("mcp") {
            mcp.push(name);
        } else if lower == "bash" || lower == "shell" || lower.starts_with("exec") {
            shell.push(name);
        } else if matches!(
            lower.as_str(),
            "read" | "write" | "edit" | "multiedit" | "glob" | "grep" | "ls"
        ) || lower.starts_with("file")
            || lower.starts_with("read_")
            || lower.starts_with("write_")
        {
            filesystem.push(name);
        } else if lower.starts_with("git") || lower.starts_with("diff") || lower == "commit" {
            git.push(name);
        } else if lower.starts_with("web") || lower.starts_with("fetch") || lower.starts_with("http") {
            web.push(name);
        } else if lower.starts_with("memory") || lower.starts_with("recall") || lower.starts_with("remember") {
            memory.push(name);
        } else {
            other.push(name);
        }
    }

    for bucket in [
        &mut filesystem,
        &mut shell,
        &mut git,
        &mut web,
        &mut memory,
        &mut mcp,
        &mut other,
    ] {
        bucket.sort();
    }

    vec![
        ("Filesystem", filesystem),
        ("Shell", shell),
        ("Git", git),
        ("Web", web),
        ("Memory", memory),
        ("MCP", mcp),
        ("Other", other),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn groups_filesystem_tools_by_exact_name_and_prefix() {
        let g = group_tools(&names(&["Read", "Write", "Edit", "Glob", "file_read_bytes"]));
        let fs = &g[0];
        assert_eq!(fs.0, "Filesystem");
        assert_eq!(fs.1.len(), 5);
    }

    #[test]
    fn mcp_prefix_puts_tools_in_mcp_bucket() {
        let g = group_tools(&names(&["mcp_resource", "mcpClient"]));
        let mcp_bucket = g.iter().find(|(n, _)| *n == "MCP").unwrap();
        assert_eq!(mcp_bucket.1.len(), 2);
    }

    #[test]
    fn unknown_tools_fall_into_other_not_dropped() {
        // Lenient bucketing: a tool we can't classify still appears.
        let g = group_tools(&names(&["xyz_custom_tool"]));
        let other = g.iter().find(|(n, _)| *n == "Other").unwrap();
        assert_eq!(other.1.len(), 1);
        assert_eq!(other.1[0], "xyz_custom_tool");
    }

    #[test]
    fn bucket_entries_are_sorted_alphabetically() {
        let g = group_tools(&names(&["Write", "Read", "Edit"]));
        let fs = &g[0].1;
        // Sort check — the output order shouldn't depend on input order.
        assert_eq!(fs, &vec!["Edit", "Read", "Write"]);
    }

    #[test]
    fn empty_input_returns_all_empty_buckets() {
        let g = group_tools(&[]);
        for (_, bucket) in g {
            assert!(bucket.is_empty());
        }
    }
}
