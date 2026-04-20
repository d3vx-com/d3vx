//! Agent slash commands: vex, spawn, compact, thinking

use anyhow::Result;

use super::*;

pub fn handle_vex(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        app.add_system_message(
            "Usage: /vex <task description>\n       /vex list  — show running background tasks",
        );
        return Ok(());
    }

    if args[0].eq_ignore_ascii_case("list") {
        return super::discovery::handle_vex_list(app);
    }

    let description = args.join(" ");
    app.start_vex_task(&description)
}

pub fn handle_compact(app: &mut App, args: &[&str]) -> Result<()> {
    let keep_last = args
        .first()
        .and_then(|a| a.parse::<usize>().ok())
        .unwrap_or(20);

    if let Some(agent) = &app.agents.agent_loop {
        let agent = agent.clone();

        // Compact the agent's conversation history
        let removed = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(agent.compact_history(keep_last))
        });

        if removed > 0 {
            // Synchronize the local UI messages list
            if app.session.messages.len() > keep_last + 1 {
                let first = app.session.messages.remove(0);
                while app.session.messages.len() > keep_last {
                    app.session.messages.remove(0);
                }
                app.session.messages.insert(0, first);
            }
            app.add_system_message(&format!(
                "Conversation compacted. Removed {} messages. Kept the first message and the last {} messages.",
                removed, keep_last
            ));
        } else {
            app.add_system_message("Conversation is already compact.");
        }
    } else {
        app.add_system_message("Cannot compact: No active agent loop.");
    }
    Ok(())
}

pub fn handle_spawn(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        app.add_system_message("Usage: /spawn [task description]");
        return Ok(());
    }

    let task = args.join(" ");
    app.add_system_message(&format!("Spawning sub-agent for: '{}'", task));

    // Prepare agent config
    let config = crate::agent::AgentConfig {
        model: app
            .model
            .clone()
            .unwrap_or_else(|| app.config.model.clone()),
        system_prompt: crate::agent::prompt::build_system_prompt_with_options(
            &app.cwd.as_deref().unwrap_or("."),
            Some(&crate::agent::prompt::Role::Executor),
            false,
        ),
        parent_session_id: app.agents.agent_loop.as_ref().and_then(|agent| {
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let config = agent.config.read().await;
                    Some(config.session_id.clone())
                })
            })
        }),
        allow_parallel_spawn: false,
        plan_mode: app.ui.plan_mode,
        ..Default::default()
    };

    // Need to get provider and tool coordinator from App
    if let Some(provider) = &app.provider {
        match tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(app.subagents.spawn(
                task.clone(),
                config,
                provider.clone(),
                app.tools.tool_coordinator.clone(),
                None,                               // Use default role
                app.agents.parallel_agents_enabled, // Inline mode based on config
            ))
        }) {
            Ok((id, rx)) => {
                // Add to inline agents list for UI display
                app.add_inline_agent(id.clone(), task.clone());

                app.add_system_message(&format!(
                    "Sub-agent spawned successfully! ID: {}",
                    &id[..8]
                ));
                app.spawn_agent_forwarder(id, rx);
            }
            Err(e) => {
                app.add_system_message(&format!("Error spawning sub-agent: {}", e));
            }
        }
    } else {
        app.add_system_message(
            "Cannot spawn sub-agent: No LLM provider available in standalone mode.",
        );
    }

    // For now, let's just list active agents
    let agents = tokio::task::block_in_place(|| {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(app.subagents.list())
    });
    if !agents.is_empty() {
        let mut list_text = String::from("\nActive Sub-agents:\n");
        for agent in agents {
            list_text.push_str(&format!(
                "  - [{}] {} ({:?})\n",
                &agent.id[..8],
                agent.task,
                agent.status
            ));
        }
        app.add_system_message(&list_text);
    }

    Ok(())
}

pub fn handle_thinking(app: &mut App, args: &[&str]) -> Result<()> {
    let agent = match &app.agents.agent_loop {
        Some(a) => a.clone(),
        None => {
            app.add_system_message("Cannot configure thinking: No active agent loop.");
            return Ok(());
        }
    };

    if args.is_empty() {
        let (enabled, budget) = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let config = rt.block_on(agent.config.read());
            (config.thinking_enabled, config.thinking_budget)
        });

        let status = if enabled { "ENABLED" } else { "DISABLED" };
        let budget_str = budget
            .map(|b| b.to_string())
            .unwrap_or_else(|| "default".to_string());
        app.add_system_message(&format!(
            "Thinking Mode: {}\nBudget: {}\n\nUse `/thinking on`, `/thinking off`, or `/thinking <budget>` to change.",
            status, budget_str
        ));
        return Ok(());
    }

    // Now handle updates
    let mut message = String::new();
    tokio::task::block_in_place(|| {
        let rt = tokio::runtime::Handle::current();
        let mut config = rt.block_on(agent.config.write());

        match args[0] {
            "on" => {
                config.thinking_enabled = true;
                message = "Thinking Mode: ENABLED (Default budget)".to_string();
            }
            "off" => {
                config.thinking_enabled = false;
                message = "Thinking Mode: DISABLED".to_string();
            }
            _ => {
                if let Ok(budget) = args[0].parse::<u32>() {
                    config.thinking_enabled = true;
                    config.thinking_budget = Some(budget);
                    message = format!("Thinking Mode: ENABLED (Budget: {} tokens)", budget);
                } else {
                    message = "Usage: /thinking [on/off/budget]".to_string();
                }
            }
        }
    });

    app.add_system_message(&message);
    Ok(())
}
