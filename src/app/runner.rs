//! Main Application Run Loop
//!
//! Contains the primary event loop that drives the TUI application,
//! including event handling, IPC polling, rendering, and cleanup.

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

use crate::app::App;
use crate::event::{Event, EventHandler};
use crate::tools::McpTool;

impl App {
    /// Run the main application loop
    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<()> {
        // Set up event handler
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(1024);
        self.set_event_tx(event_tx.clone());

        // Spawn forwarders for any pending agent receivers (including the main "home" agent)
        let pending: Vec<(
            String,
            tokio::sync::broadcast::Receiver<crate::agent::AgentEvent>,
        )> = self.agents.pending_agent_receivers.drain().collect();
        for (id, rx) in pending {
            self.spawn_agent_forwarder(id, rx);
        }
        let event_handler = EventHandler::new(event_tx.clone());
        event_handler.spawn()?;

        // Register SpawnAgent tool with the application's event channel
        if self.provider.is_some() {
            self.tools
                .tool_coordinator
                .register_handler(Arc::new(
                    crate::agent::tool_coordinator::SubAgentToolHandler::new(event_tx.clone()),
                ))
                .await;

            // Register SendInboxMessageTool with the application's event channel
            self.tools
                .tool_coordinator
                .register_tool(crate::tools::SendInboxMessageTool::with_sender(
                    event_tx.clone(),
                ))
                .await;
        }

        // Start Orchestrator Watchdog
        self.orchestrator.clone().start_crash_watchdog().await;

        // Spawn parallel event handler - poll the channel and send events through event_tx
        if let Some(mut spawn_rx) = self.agents.spawn_parallel_receiver.take() {
            let event_tx_clone = event_tx.clone();
            tokio::spawn(async move {
                tracing::info!("SpawnParallel event forwarder: started");
                while let Some(spawn_event) = spawn_rx.recv().await {
                    tracing::info!(
                        "SpawnParallel event forwarder: received event with {} tasks",
                        spawn_event.tasks.len()
                    );
                    if event_tx_clone
                        .send(Event::SpawnParallel(spawn_event))
                        .await
                        .is_err()
                    {
                        tracing::error!("SpawnParallel event forwarder: failed to send event");
                        break;
                    }
                    tracing::info!("SpawnParallel event forwarder: event sent successfully");
                }
                tracing::info!("SpawnParallel event forwarder: ended");
            });
        } else {
            tracing::warn!("SpawnParallel event forwarder: receiver was None");
        }

        // Set up animation ticker
        let ticker_tx = event_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                interval.tick().await;
                if ticker_tx.send(Event::Tick).await.is_err() {
                    break;
                }
            }
        });

        // Initialize MCP servers from config
        let mcp_config = self.config.mcp.clone();
        for (name, server_config) in mcp_config.servers {
            let mcp_manager = self.mcp_manager.clone();
            let tool_coordinator = self.tools.tool_coordinator.clone();
            tokio::spawn(async move {
                if let Err(e) = mcp_manager.add_server(name.clone(), server_config).await {
                    tracing::error!("Failed to add MCP server '{}': {}", name, e);
                    return;
                }

                // Register tools from this server
                let tools = mcp_manager.list_all_tools().await;
                for (srv_name, tool_def) in tools {
                    if srv_name == name {
                        tool_coordinator
                            .register_tool(McpTool::new(srv_name, mcp_manager.clone(), tool_def))
                            .await;
                    }
                }
            });
        }

        info!("App started");

        // Main loop
        while !self.should_quit {
            // Update application state
            self.update();
            self.sync_agent_context().await?;

            // Use select! to handle UI events (which now include tagged Agent events)
            tokio::select! {
                // Poll UI events (keyboard, mouse, tick, tagged agent events, spawn parallel)
                Some(event) = event_rx.recv() => {
                    self.handle_event(event).await?;
                }
            }

            // Handle IPC events - collect first to avoid double borrow
            let ipc_events: Vec<_> = {
                if let Some(ref client) = self.ipc_client {
                    let mut events = Vec::new();
                    while let Some(event) = client.try_recv_event() {
                        events.push(event);
                    }
                    events
                } else {
                    Vec::new()
                }
            };
            for (event, value) in ipc_events {
                if let Ok(ipc_event) = crate::ipc::parse_event(event, value) {
                    self.handle_ipc_event(ipc_event).await?;
                }
            }

            // Render only if something changed
            if self.needs_redraw {
                terminal.draw(|f| self.render(f))?;
                self.needs_redraw = false;
            }
        }

        // Final save on exit for all active agents
        for _agent in self.agents.workspace_agents.values() {
            // Need a way to save specific sessions... for now just the active one is fine
        }
        if self.agents.agent_loop.is_some() {
            let _ = self.save_current_session().await;
        }

        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let mcp_manager = self.mcp_manager.clone();
        let ipc_handle = self.ipc_handle.take();

        // Try to get a runtime handle; if we're inside an async context,
        // use `spawn` (fire-and-forget). If not, create a short-lived
        // blocking runtime for cleanup.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // We're inside a tokio runtime — spawn the cleanup as a
                // detached task so we never call block_on from within an
                // async context (which would panic).
                handle.spawn(async move {
                    mcp_manager.shutdown_all().await;
                    if let Some(h) = ipc_handle {
                        let _ = h.shutdown().await;
                    }
                });
            }
            Err(_) => {
                // No runtime — build a temporary one for cleanup.
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    let _ = rt.block_on(async move {
                        mcp_manager.shutdown_all().await;
                        if let Some(h) = ipc_handle {
                            let _ = h.shutdown().await;
                        }
                    });
                }
            }
        }
    }
}
