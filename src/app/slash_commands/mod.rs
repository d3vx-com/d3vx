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
    pub handler: fn(&mut App, args: &[&str]) -> Result<()>,
}

/// Available slash commands
pub const SLASH_COMMANDS: &[SlashCommand] = &[
    // Core
    SlashCommand {
        name: "help",
        description: "Show available slash commands",
        usage: "/help",
        handler: show_help,
    },
    SlashCommand {
        name: "clear",
        description: "Clear conversation history",
        usage: "/clear",
        handler: handle_clear,
    },
    SlashCommand {
        name: "compact",
        description: "Compact conversation to reduce context size",
        usage: "/compact",
        handler: handle_compact,
    },
    SlashCommand {
        name: "status",
        description: "Show session status and statistics",
        usage: "/status",
        handler: handle_status,
    },
    SlashCommand {
        name: "cost",
        description: "Show token usage and estimated cost",
        usage: "/cost",
        handler: handle_cost,
    },
    // Modes
    SlashCommand {
        name: "verbose",
        description: "Toggle verbose mode (expanded tool calls)",
        usage: "/verbose",
        handler: handle_verbose,
    },
    SlashCommand {
        name: "power",
        description: "Toggle Power Mode (advanced sidebars and metrics)",
        usage: "/power",
        handler: handle_power,
    },
    SlashCommand {
        name: "vibe",
        description: "Engage Vibe Mode (hides metrics for pure focus)",
        usage: "/vibe",
        handler: handle_vibe,
    },
    // Session
    SlashCommand {
        name: "undo",
        description: "Enter undo picker mode",
        usage: "/undo",
        handler: handle_undo,
    },
    // Navigation
    SlashCommand {
        name: "board",
        description: "Enter kanban board mode",
        usage: "/board",
        handler: handle_board,
    },
    SlashCommand {
        name: "list",
        description: "Enter task list mode",
        usage: "/list",
        handler: handle_list,
    },
    SlashCommand {
        name: "agents",
        description: "Enter agent monitoring mode",
        usage: "/agents",
        handler: handle_agents,
    },
    // Modes
    SlashCommand {
        name: "model",
        description: "Show or change current model",
        usage: "/model [model_name]",
        handler: handle_model,
    },
    SlashCommand {
        name: "mode",
        description: "Show or switch chat focus mode",
        usage: "/mode [chat|build|plan|docs|test|review]",
        handler: handle_mode,
    },
    // Session
    SlashCommand {
        name: "resume",
        description: "Resume a previous session",
        usage: "/resume [session_id]",
        handler: handle_resume,
    },
    SlashCommand {
        name: "export",
        description: "Export conversation to file",
        usage: "/export [filename]",
        handler: handle_export,
    },
    // Setup
    SlashCommand {
        name: "init",
        description: "Analyze project and create .d3vx/context.md",
        usage: "/init",
        handler: handle_init,
    },
    // Modes
    SlashCommand {
        name: "plan",
        description: "Toggle read-only plan mode (blocks write/edit/bash)",
        usage: "/plan",
        handler: handle_plan,
    },
    // Agent
    SlashCommand {
        name: "vex",
        description: "Start a background task in an isolated worktree",
        usage: "/vex [task description]",
        handler: handle_vex,
    },
    // Tools
    SlashCommand {
        name: "expand",
        description: "Expand/Collapse tool output (index or 'none')",
        usage: "/expand [index/none]",
        handler: handle_expand,
    },
    // Git
    SlashCommand {
        name: "commit",
        description: "AI-generated git commit for staged changes",
        usage: "/commit",
        handler: handle_commit,
    },
    SlashCommand {
        name: "pr",
        description: "AI-generated pull request description",
        usage: "/pr",
        handler: handle_pr,
    },
    // Tools
    SlashCommand {
        name: "image",
        description: "Attach an image to the next prompt",
        usage: "/image <path>",
        handler: handle_image,
    },
    // Setup
    SlashCommand {
        name: "doctor",
        description: "Run diagnostic checks on the environment",
        usage: "/doctor",
        handler: handle_doctor,
    },
    SlashCommand {
        name: "setup",
        description: "Interactive setup wizard for configuration",
        usage: "/setup",
        handler: handle_setup,
    },
    SlashCommand {
        name: "pricing",
        description: "Show pricing and budget information",
        usage: "/pricing",
        handler: handle_pricing,
    },
    // Agent
    SlashCommand {
        name: "spawn",
        description: "Spawn a parallel sub-agent task",
        usage: "/spawn [task description]",
        handler: handle_spawn,
    },
    SlashCommand {
        name: "thinking",
        description: "Configure LLM thinking (on/off/budget)",
        usage: "/thinking [on/off/budget]",
        handler: handle_thinking,
    },
    // Core
    SlashCommand {
        name: "quit",
        description: "Exit the application",
        usage: "/quit",
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

    // Parse command name and args
    let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
    let cmd_name = parts.first().copied().unwrap_or("");
    let args = &parts[1..];

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
