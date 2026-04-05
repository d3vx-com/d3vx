//! Mode toggle slash commands: verbose, power, vibe, plan, focus mode, model

use anyhow::Result;

use super::*;

pub fn handle_verbose(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.verbose = !app.ui.verbose;
    let state = if app.ui.verbose { "ON" } else { "OFF" };
    app.add_system_message(&format!("Verbose mode: {}", state));
    Ok(())
}

pub fn handle_power(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.power_mode = !app.ui.power_mode;
    app.ui.right_sidebar_visible = app.ui.power_mode;
    app.ui.verbose = app.ui.power_mode;

    let state = if app.ui.power_mode {
        "ON (Advanced telemetry enabled)"
    } else {
        "OFF (Vibe mode restoring)"
    };
    app.add_system_message(&format!("Power Mode: {}", state));
    Ok(())
}

pub fn handle_vibe(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.power_mode = false;
    app.ui.right_sidebar_visible = false;
    app.ui.verbose = false;

    app.add_system_message("Vibe Mode: ON (Telemetry hidden for maximum focus)");
    Ok(())
}

pub fn handle_plan(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.plan_mode = !app.ui.plan_mode;
    if app.ui.plan_mode {
        app.ui.mode = crate::app::state::AppMode::Plan;
        app.add_system_message(
            "PLAN MODE ACTIVATED\nWrite, edit, and bash tools are now blocked.\nThe agent can only read files and formulate plans.",
        );
    } else {
        app.ui.mode = crate::app::state::AppMode::Chat;
        app.add_system_message("EXECUTION MODE ACTIVATED\nAll tools are now available.");
    }
    Ok(())
}

pub fn handle_mode(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        app.add_system_message(&format!(
            "Current focus mode: {}. Available modes: chat, build, plan, docs, test, review.",
            app.ui.focus_mode.label()
        ));
        return Ok(());
    }

    let mode = match args[0].to_ascii_lowercase().as_str() {
        "chat" => crate::app::FocusMode::Chat,
        "build" => crate::app::FocusMode::Build,
        "plan" => crate::app::FocusMode::Plan,
        "docs" => crate::app::FocusMode::Docs,
        "test" => crate::app::FocusMode::Test,
        "review" => crate::app::FocusMode::Review,
        _ => {
            app.add_system_message(
                "Unknown focus mode. Use one of: chat, build, plan, docs, test, review.",
            );
            return Ok(());
        }
    };

    app.ui.focus_mode = mode;
    app.add_system_message(&format!(
        "Focus mode set to {}. {}",
        mode.label(),
        mode.hint()
    ));
    Ok(())
}

pub fn handle_model(app: &mut App, args: &[&str]) -> Result<()> {
    if let Some(new_model) = args.first() {
        app.model = Some(new_model.to_string());
        app.add_system_message(&format!("Model switched to: {}", new_model));
    } else {
        // Open the dynamic picker
        app.ui.show_model_picker = true;
        app.ui.model_picker_filter.clear();
        app.ui.model_picker_selected_index = 0;
        app.ui.model_picker_entering_api_key = false;
        app.ui.model_picker_api_key_input.clear();

        app.add_system_message("Opening model configuration...");

        // Trigger a refresh of models in case they haven't been fetched yet
        // Note: Background fetch is handled in App::new initialization
        let _event_tx = app.event_tx.clone();
    }
    Ok(())
}
