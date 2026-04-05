//! Navigation slash commands: board, list, agents

use anyhow::Result;

use super::*;

pub fn handle_board(app: &mut App, args: &[&str]) -> Result<()> {
    let _ = app.refresh_task_views();

    // Handle view mode argument
    if let Some(mode) = args.first() {
        match mode.to_lowercase().as_str() {
            "list" | "l" => {
                app.ui.mode = AppMode::List;
                app.add_system_message("Switched to list view. Use /board to return to kanban.");
            }
            "agents" | "a" | "tasks" => {
                app.ui.mode = AppMode::Board;
                app.ui.right_sidebar_visible = true;
                app.add_system_message(
                    "Showing task board with agents. Use /board list for compact view.",
                );
            }
            _ => {
                app.ui.mode = AppMode::Board;
                app.add_system_message("Switched to Kanban Board view.");
            }
        }
    } else {
        app.ui.mode = AppMode::Board;
        app.add_system_message("Switched to Kanban Board view.");
    }
    Ok(())
}

pub fn handle_list(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.mode = AppMode::List;
    app.add_system_message("Switched to list view. Use /board for kanban view.");
    Ok(())
}

pub fn handle_agents(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.right_sidebar_visible = true;
    app.add_system_message("Agents panel opened. Use /board to see full kanban view.");
    Ok(())
}
