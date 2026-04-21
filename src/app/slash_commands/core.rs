//! Core slash commands: help, clear, quit, status, cost

use anyhow::Result;

use super::*;

pub fn show_help(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.enter_overlay(crate::app::state::Overlay::Help);
    app.ui.help_scroll = 0;
    Ok(())
}

pub fn handle_clear(app: &mut App, _args: &[&str]) -> Result<()> {
    app.session.messages.clear();
    app.ui.scroll_offset = 0;
    app.ui.show_welcome = true;
    app.add_system_message("Conversation history cleared.");

    // Trigger save to reflect cleared history in DB
    if app.agents.agent_loop.is_some() {
        if let Some(tx) = &app.event_tx {
            let tx = tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(crate::event::Event::SaveSession).await;
            });
        }
    }
    Ok(())
}

pub fn handle_quit(app: &mut App, _args: &[&str]) -> Result<()> {
    app.should_quit = true;
    Ok(())
}

pub fn handle_status(app: &mut App, _args: &[&str]) -> Result<()> {
    let usage = &app.session.token_usage;
    let mut status = String::from("Status:\n\n");

    status.push_str(&format!(
        "  Working Dir: {}\n",
        app.cwd.as_deref().unwrap_or(".")
    ));
    status.push_str(&format!(
        "  Model: {}\n",
        app.model.as_deref().unwrap_or("default")
    ));
    status.push_str(&format!("  Messages: {}\n", app.session.messages.len()));
    status.push_str(&format!(
        "  Connected: {}\n",
        if app.agents.is_connected { "Yes" } else { "No" }
    ));

    status.push_str("\n  Token Usage:\n");
    status.push_str(&format!("    Input:  {}\n", usage.input_tokens));
    status.push_str(&format!("    Output: {}\n", usage.output_tokens));

    if let Some(cost) = usage.total_cost {
        status.push_str(&format!("    Cost: ${:.4}\n", cost));
    }

    if app.session.thinking.is_thinking {
        status.push_str("\n  Status: Thinking...\n");
    }

    app.add_system_message(&status);
    Ok(())
}

pub fn handle_cost(app: &mut App, _args: &[&str]) -> Result<()> {
    let usage = &app.session.token_usage;
    let mut cost_text = String::from("Token Usage:\n\n");
    cost_text.push_str(&format!("  Input Tokens: {}\n", usage.input_tokens));
    cost_text.push_str(&format!("  Output Tokens: {}\n", usage.output_tokens));

    if let Some(cache_read) = usage.cache_read_tokens {
        cost_text.push_str(&format!("  Cache Read: {}\n", cache_read));
    }

    if let Some(cost) = usage.total_cost {
        cost_text.push_str(&format!("  Estimated Cost: ${:.4}\n", cost));
    }

    app.add_system_message(&cost_text);
    Ok(())
}
