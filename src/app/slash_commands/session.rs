//! Session slash commands: undo, resume, export

use anyhow::Result;

use super::*;

pub fn handle_undo(app: &mut App, _args: &[&str]) -> Result<()> {
    use crate::ui::widgets::UndoPicker;

    // Create undo picker from current messages and file change log
    app.undo_picker = Some(UndoPicker::from_messages(
        &app.session.messages,
        &app.session.file_change_log,
    ));
    app.ui.enter_overlay(crate::app::state::Overlay::UndoPicker);
    Ok(())
}

pub fn handle_resume(app: &mut App, args: &[&str]) -> Result<()> {
    if let Some(session_id_str) = args.first() {
        let session_id = session_id_str.to_string();

        app.add_system_message(&format!("Resuming session {}...", session_id));

        // Use block_in_place to run the async resume_session
        tokio::task::block_in_place(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(app.resume_session(&session_id))
        })?;
    } else {
        // No ID provided, show sessions for this directory
        let sessions = {
            let db_handle = app
                .db
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database not available"))?;
            let db = db_handle.lock();
            let store = crate::store::session::SessionStore::from_connection(db.connection());

            let options = crate::store::session::SessionListOptions {
                project_path: app.cwd.clone(),
                limit: Some(20),
                ..Default::default()
            };

            store.list(options)?
        };

        if sessions.is_empty() {
            app.add_system_message("No previous sessions found for this directory.");
            return Ok(());
        }

        app.session_picker = Some(crate::ui::widgets::SessionPicker::new(sessions));
        app.ui.enter_overlay(crate::app::state::Overlay::SessionPicker);
    }
    Ok(())
}

pub fn handle_export(app: &mut App, args: &[&str]) -> Result<()> {
    if app.session.messages.is_empty() {
        app.add_system_message("Nothing to export — conversation is empty.");
        return Ok(());
    }

    // Determine output filename
    let filename = if let Some(name) = args.first() {
        let name = name.to_string();
        if name.ends_with(".md") || name.ends_with(".json") {
            name
        } else {
            format!("{}.md", name)
        }
    } else {
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        format!("d3vx-export-{}.md", ts)
    };

    // Build markdown content
    let mut md = String::with_capacity(4096);
    md.push_str("# d3vx Conversation Export\n\n");
    md.push_str(&format!(
        "**Exported**: {}  \n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    if let Some(ref cwd) = app.cwd {
        md.push_str(&format!("**Project**: `{}`  \n", cwd));
    }
    if let Some(ref model) = app.model {
        md.push_str(&format!("**Model**: `{}`  \n", model));
    }
    md.push_str(&format!(
        "**Messages**: {}  \n\n---\n\n",
        app.session.messages.len()
    ));

    for msg in &app.session.messages {
        let role_label = match msg.role {
            crate::ipc::MessageRole::User => "## User",
            crate::ipc::MessageRole::Assistant => "## Assistant",
            crate::ipc::MessageRole::System => "## System",
            crate::ipc::MessageRole::Shell => "## Shell",
        };
        md.push_str(&format!("{}\n", role_label));
        md.push_str(&format!("*{}*\n\n", msg.timestamp.format("%H:%M:%S")));

        // Shell messages get special formatting
        if msg.role == crate::ipc::MessageRole::Shell {
            if let Some(ref cmd) = msg.shell_cmd {
                md.push_str(&format!("```bash\n$ {}\n```\n\n", cmd));
            }
            if !msg.content.is_empty() {
                md.push_str(&format!("```\n{}\n```\n\n", msg.content));
            }
        } else if !msg.content.is_empty() {
            md.push_str(&msg.content);
            md.push_str("\n\n");
        }

        // Append tool calls
        for tc in &msg.tool_calls {
            md.push_str(&format!("### Tool: `{}`\n\n", tc.name));
            md.push_str(&format!(
                "```json\n{}\n```\n\n",
                serde_json::to_string_pretty(&tc.input).unwrap_or_default()
            ));
            if let Some(ref output) = tc.output {
                let truncated = if output.len() > 2000 {
                    &output[..2000]
                } else {
                    output
                };
                md.push_str(&format!(
                    "<details><summary>Output ({} chars)</summary>\n\n```\n{}\n```\n\n</details>\n\n",
                    output.len(),
                    truncated
                ));
            }
        }

        md.push_str("---\n\n");
    }

    // Write file
    let path = std::path::Path::new(&filename);
    match std::fs::write(path, &md) {
        Ok(_) => {
            let abs_path = std::fs::canonicalize(path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| filename.clone());
            app.add_system_message(&format!("Conversation exported to: {}", abs_path));
        }
        Err(e) => {
            app.add_system_message(&format!("Export failed: {}", e));
        }
    }
    Ok(())
}
