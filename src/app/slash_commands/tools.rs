//! Tool-related slash commands: expand, image, commit, pr

use anyhow::Result;

use super::*;

pub fn handle_expand(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        // Toggle expansion for ALL tools in the last message that has tools
        if let Some(msg) = app
            .session
            .messages
            .iter()
            .rev()
            .find(|m| !m.tool_calls.is_empty())
        {
            let all_expanded = msg
                .tool_calls
                .iter()
                .all(|tc| app.tools.expanded_tool_calls.contains(&tc.id));

            if all_expanded {
                for tc in &msg.tool_calls {
                    app.tools.expanded_tool_calls.remove(&tc.id);
                }
                app.add_system_message("Collapsed all tools in the last message.");
            } else {
                for tc in &msg.tool_calls {
                    app.tools.expanded_tool_calls.insert(tc.id.clone());
                }
                app.add_system_message(&format!(
                    "Expanded {} tool calls in the last message.",
                    msg.tool_calls.len()
                ));
            }
        } else {
            app.add_system_message("No tool calls found to expand.");
        }
        return Ok(());
    }

    if args[0] == "none" || args[0] == "off" || args[0] == "clear" {
        app.tools.expanded_tool_calls.clear();
        app.add_system_message("Collapsed all tool calls globally.");
        return Ok(());
    }

    if args[0] == "all" || args[0] == "on" {
        for msg in &app.session.messages {
            for tc in &msg.tool_calls {
                app.tools.expanded_tool_calls.insert(tc.id.clone());
            }
        }
        app.add_system_message("Expanded all tool calls in history.");
        return Ok(());
    }

    // Try to expand by index in the last message
    if let Ok(idx) = args[0].parse::<usize>() {
        if let Some(msg) = app
            .session
            .messages
            .iter()
            .rev()
            .find(|m| !m.tool_calls.is_empty())
        {
            if idx > 0 && idx <= msg.tool_calls.len() {
                let tc_id = msg.tool_calls[idx - 1].id.clone();
                if app.tools.expanded_tool_calls.contains(&tc_id) {
                    app.tools.expanded_tool_calls.remove(&tc_id);
                    app.add_system_message(&format!(
                        "Collapsed tool #{} ({}).",
                        idx,
                        msg.tool_calls[idx - 1].name
                    ));
                } else {
                    app.tools.expanded_tool_calls.insert(tc_id);
                    app.add_system_message(&format!(
                        "Expanded tool #{} ({}).",
                        idx,
                        msg.tool_calls[idx - 1].name
                    ));
                }
            } else {
                app.add_system_message(&format!(
                    "Tool index {} out of range (1-{}).",
                    idx,
                    msg.tool_calls.len()
                ));
            }
        } else {
            app.add_system_message("No recent tool calls found.");
        }
    } else {
        app.add_system_message("Usage: /expand [index], /expand all, or /expand none");
    }

    Ok(())
}

pub fn handle_image(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        app.add_system_message("Usage: /image <path/to/image.png>");
        return Ok(());
    }

    let path = args.join(" ");
    let file_path = if path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        std::path::PathBuf::from(path.replacen('~', &home, 1))
    } else {
        std::path::PathBuf::from(&path)
    };

    if !file_path.exists() {
        app.add_system_message(&format!("Image file not found: {}", path));
        return Ok(());
    }

    app.session.pending_images.push(file_path.clone());
    app.add_system_message(&format!(
        "Attached image {}. It will be sent with your next message.",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    Ok(())
}

pub fn handle_commit(app: &mut App, _args: &[&str]) -> Result<()> {
    app.add_system_message("Analyzing staged changes for AI commit...");
    // Special message to trigger AI commit logic
    if let Some(tx) = &app.event_tx {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(crate::event::Event::SendMessage(
                "Draft a concise and descriptive git commit message based on the currently staged changes. Use the following format: <type>(<scope>): <subject>"
                    .to_string(),
            ))
            .await;
        });
    }
    Ok(())
}

pub fn handle_pr(app: &mut App, _args: &[&str]) -> Result<()> {
    app.add_system_message("Analyzing branch changes for AI PR description...");
    // Special message to trigger AI PR logic
    if let Some(tx) = &app.event_tx {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(crate::event::Event::SendMessage(
                    "Draft a pull request description for the current branch compared to main. Highlight key changes, design decisions, and testing performed."
                        .to_string(),
                ))
                .await;
        });
    }
    Ok(())
}
