//! Slash Commands Module
//!
//! Organized by category into submodules:
//! - `core` — help, clear, quit, status, cost
//! - `setup` — doctor, setup, init, pricing
//! - `navigation` — board, list, agents
//! - `agent` — spawn, vex, compact, thinking
//! - `session` — undo, resume, export
//! - `modes` — verbose, power, vibe, plan, mode, model
//! - `tools` — expand, image, commit, pr

mod agent;
mod core;
mod discovery;
mod modes;
mod navigation;
mod session;
mod setup;
mod tools;

use anyhow::Result;
use tracing::debug;

use super::{App, AppMode};

// Re-export all handler functions for use in the command registry
pub use agent::{handle_compact, handle_spawn, handle_thinking, handle_vex};
pub use core::{handle_clear, handle_cost, handle_quit, handle_status, show_help};
pub use discovery::{daemon_is_running, handle_daemon, handle_dashboard};
pub use modes::{
    handle_mode, handle_model, handle_plan, handle_power, handle_verbose, handle_vibe,
};
pub use navigation::{handle_agents, handle_board, handle_list};
pub use session::{handle_export, handle_resume, handle_undo};
pub use setup::{handle_doctor, handle_init, handle_pricing, handle_setup};
pub use tools::{handle_commit, handle_expand, handle_image, handle_pr};

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION: Command Registry
// ═══════════════════════════════════════════════════════════════════════════════

/// Slash command definition
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    /// Category label used by `/help` to group commands. Same string
    /// on multiple commands = same section. Keep these stable — they
    /// are shown to users and act as the mental map of the CLI.
    pub category: &'static str,
    pub handler: fn(&mut App, args: &[&str]) -> Result<()>,
}

/// Category ordering for `/help`. The help modal iterates this list
/// and, for each category, prints every command with a matching
/// `category` field. A command whose category isn't in this list is
/// dropped from help — intentional, so we don't silently surface
/// half-wired commands.
pub const CATEGORY_ORDER: &[&str] = &[
    "Discovery",
    "Modes & views",
    "Agents & tasks",
    "Session",
    "Git & content",
    "Setup",
];

/// Available slash commands. Order here is the order shown in the
/// live palette (typing `/` with no filter). The `/help` modal
/// regroups these by `category` using `CATEGORY_ORDER`.
pub const SLASH_COMMANDS: &[SlashCommand] = &[
    // ── Discovery ────────────────────────────────────────────────
    SlashCommand {
        name: "help",
        description: "Show all commands and keyboard shortcuts",
        usage: "/help",
        category: "Discovery",
        handler: show_help,
    },
    SlashCommand {
        name: "dashboard",
        description: "Open the live web dashboard in your browser",
        usage: "/dashboard",
        category: "Discovery",
        handler: handle_dashboard,
    },
    SlashCommand {
        name: "daemon",
        description: "Show background daemon status",
        usage: "/daemon",
        category: "Discovery",
        handler: handle_daemon,
    },
    // ── Modes & views ────────────────────────────────────────────
    SlashCommand {
        name: "board",
        description: "Enter kanban board view (Esc to exit)",
        usage: "/board",
        category: "Modes & views",
        handler: handle_board,
    },
    SlashCommand {
        name: "list",
        description: "Enter task list view (Esc to exit)",
        usage: "/list",
        category: "Modes & views",
        handler: handle_list,
    },
    SlashCommand {
        name: "agents",
        description: "Open the agents monitoring panel",
        usage: "/agents",
        category: "Modes & views",
        handler: handle_agents,
    },
    SlashCommand {
        name: "mode",
        description: "Switch chat focus mode",
        usage: "/mode [chat|build|plan|docs|test|review]",
        category: "Modes & views",
        handler: handle_mode,
    },
    SlashCommand {
        name: "model",
        description: "Show or change the current LLM model",
        usage: "/model [model_name]",
        category: "Modes & views",
        handler: handle_model,
    },
    SlashCommand {
        name: "verbose",
        description: "Toggle verbose mode (expanded tool calls)",
        usage: "/verbose",
        category: "Modes & views",
        handler: handle_verbose,
    },
    SlashCommand {
        name: "power",
        description: "Toggle Power Mode — show advanced sidebars + metrics",
        usage: "/power",
        category: "Modes & views",
        handler: handle_power,
    },
    SlashCommand {
        name: "vibe",
        description: "Toggle Vibe Mode — hide metrics for pure focus",
        usage: "/vibe",
        category: "Modes & views",
        handler: handle_vibe,
    },
    SlashCommand {
        name: "plan",
        description: "Toggle read-only plan mode (blocks write/edit/bash)",
        usage: "/plan",
        category: "Modes & views",
        handler: handle_plan,
    },
    // ── Agents & tasks ───────────────────────────────────────────
    SlashCommand {
        name: "vex",
        description: "Run a task in an isolated worktree, or list running ones",
        usage: "/vex [list | task description]",
        category: "Agents & tasks",
        handler: handle_vex,
    },
    SlashCommand {
        name: "spawn",
        description: "Spawn a parallel sub-agent for a task",
        usage: "/spawn [task description]",
        category: "Agents & tasks",
        handler: handle_spawn,
    },
    SlashCommand {
        name: "thinking",
        description: "Configure LLM extended thinking (on/off/budget)",
        usage: "/thinking [on/off/budget]",
        category: "Agents & tasks",
        handler: handle_thinking,
    },
    // ── Session ──────────────────────────────────────────────────
    SlashCommand {
        name: "clear",
        description: "Clear the current conversation",
        usage: "/clear",
        category: "Session",
        handler: handle_clear,
    },
    SlashCommand {
        name: "compact",
        description: "Summarise history to reduce context size",
        usage: "/compact",
        category: "Session",
        handler: handle_compact,
    },
    SlashCommand {
        name: "status",
        description: "Show session stats",
        usage: "/status",
        category: "Session",
        handler: handle_status,
    },
    SlashCommand {
        name: "cost",
        description: "Show token usage and estimated cost",
        usage: "/cost",
        category: "Session",
        handler: handle_cost,
    },
    SlashCommand {
        name: "undo",
        description: "Open the undo picker",
        usage: "/undo",
        category: "Session",
        handler: handle_undo,
    },
    SlashCommand {
        name: "resume",
        description: "Resume a previous session",
        usage: "/resume [session_id]",
        category: "Session",
        handler: handle_resume,
    },
    SlashCommand {
        name: "export",
        description: "Export the conversation to a file",
        usage: "/export [filename]",
        category: "Session",
        handler: handle_export,
    },
    // ── Git & content ────────────────────────────────────────────
    SlashCommand {
        name: "commit",
        description: "AI-generated commit message for staged changes",
        usage: "/commit",
        category: "Git & content",
        handler: handle_commit,
    },
    SlashCommand {
        name: "pr",
        description: "AI-generated pull request description",
        usage: "/pr",
        category: "Git & content",
        handler: handle_pr,
    },
    SlashCommand {
        name: "expand",
        description: "Expand or collapse a tool-call output",
        usage: "/expand [index|none]",
        category: "Git & content",
        handler: handle_expand,
    },
    SlashCommand {
        name: "image",
        description: "Attach an image to your next prompt",
        usage: "/image <path>",
        category: "Git & content",
        handler: handle_image,
    },
    // ── Setup ────────────────────────────────────────────────────
    SlashCommand {
        name: "setup",
        description: "Interactive setup wizard",
        usage: "/setup",
        category: "Setup",
        handler: handle_setup,
    },
    SlashCommand {
        name: "doctor",
        description: "Diagnose your environment",
        usage: "/doctor",
        category: "Setup",
        handler: handle_doctor,
    },
    SlashCommand {
        name: "init",
        description: "Analyse the project and write .d3vx/context.md",
        usage: "/init",
        category: "Setup",
        handler: handle_init,
    },
    SlashCommand {
        name: "pricing",
        description: "Show pricing and budget information",
        usage: "/pricing",
        category: "Setup",
        handler: handle_pricing,
    },
    SlashCommand {
        name: "quit",
        description: "Exit d3vx",
        usage: "/quit",
        category: "Setup",
        handler: handle_quit,
    },
];

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION: Dispatcher
// ═══════════════════════════════════════════════════════════════════════════════

/// Try to parse and execute a slash command
pub fn try_execute_slash_command(app: &mut App, input: &str) -> Result<bool> {
    let trimmed = input.trim();

    // Check if it starts with /
    if !trimmed.starts_with('/') {
        return Ok(false);
    }

    // Parse command name and args.
    //
    // `trimmed[1..]` is safe here — we've already verified the string
    // starts with `/`, so there's at least one byte to skip. But
    // `split_whitespace()` can yield zero tokens (bare `/` or `/   `),
    // so we must not blindly slice `parts[1..]` — that's what produced
    // the earlier "range start index 1 out of range" panic when a user
    // hit Enter on an empty slash prompt.
    let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
    let cmd_name = parts.first().copied().unwrap_or("");
    let args: &[&str] = if parts.len() > 1 { &parts[1..] } else { &[] };

    // Bare `/` — treat as a no-op rather than an "Unknown command"
    // notification. The palette already showed the user their options;
    // no point echoing an error at them for closing it empty.
    if cmd_name.is_empty() {
        return Ok(true);
    }

    // Find matching command
    for cmd in SLASH_COMMANDS {
        if cmd.name == cmd_name {
            debug!(
                "Executing slash command: {} with args: {:?}",
                cmd.name, args
            );
            (cmd.handler)(app, args)?;
            return Ok(true);
        }
    }

    // Unknown command
    app.add_system_message(&format!(
        "Unknown command: /{}. Type /help for available commands.",
        cmd_name
    ));
    Ok(true)
}
