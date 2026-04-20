//! Navigation slash commands: board, list, agents

use anyhow::Result;

use super::*;

/// Shared exit-hint text. Users consistently ask "how do I get back
/// to chat?" when they first enter these views, so *every* mode-entry
/// message names Esc as the way out. Single definition so the wording
/// stays aligned across commands.
const EXIT_HINT: &str = "Esc returns to chat.";

pub fn handle_board(app: &mut App, args: &[&str]) -> Result<()> {
    let _ = app.refresh_task_views();

    if let Some(arg) = args.first() {
        match arg.to_lowercase().as_str() {
            "list" | "l" => {
                app.ui.mode = AppMode::List;
                app.add_system_message(&format!(
                    "Switched to task list. {EXIT_HINT} /board returns to kanban."
                ));
            }
            "agents" | "a" | "tasks" => {
                app.ui.mode = AppMode::Board;
                app.ui.right_sidebar_visible = true;
                app.add_system_message(&format!(
                    "Showing task board with agents sidebar. {EXIT_HINT} /board list for the compact view."
                ));
            }
            _ => {
                app.ui.mode = AppMode::Board;
                app.add_system_message(&format!("Switched to kanban board. {EXIT_HINT}"));
            }
        }
    } else {
        app.ui.mode = AppMode::Board;
        app.add_system_message(&format!("Switched to kanban board. {EXIT_HINT}"));
    }
    Ok(())
}

pub fn handle_list(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.mode = AppMode::List;
    app.add_system_message(&format!(
        "Switched to task list. {EXIT_HINT} /board for kanban."
    ));
    Ok(())
}

pub fn handle_agents(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.right_sidebar_visible = true;
    app.add_system_message("Agents panel opened. Use /board for the full kanban view.");
    Ok(())
}
