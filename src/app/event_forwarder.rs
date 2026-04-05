//! Agent Event Forwarding
//!
//! Spawns background tasks that relay agent events into the main event
//! loop, tagging each with the originating workspace ID.

use tokio::sync::mpsc;

use crate::agent::AgentEvent;
use crate::app::App;
use crate::event::Event;

impl App {
    /// Set the event sender
    pub fn set_event_tx(&mut self, tx: mpsc::Sender<Event>) {
        self.event_tx = Some(tx.clone());

        // Spawn any pending forwarders
        let pending = std::mem::take(&mut self.agents.pending_agent_receivers);
        for (id, rx) in pending {
            self.spawn_agent_forwarder(id, rx);
        }
    }

    /// Spawn a background task to forward agent events with workspace tagging
    pub fn spawn_agent_forwarder(
        &mut self,
        workspace_id: String,
        mut receiver: tokio::sync::broadcast::Receiver<AgentEvent>,
    ) {
        if let Some(tx) = &self.event_tx {
            let tx = tx.clone();
            let ws_id = workspace_id.clone();
            tokio::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(event) => {
                            if tx
                                .send(Event::AgentInWorkspace(ws_id.clone(), event))
                                .await
                                .is_err()
                            {
                                tracing::debug!(
                                    "Agent forwarder: event channel closed for {}",
                                    ws_id
                                );
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            tracing::debug!("Agent forwarder: broadcast closed for {}", ws_id);
                            break;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            // Receiver lagged but channel still open - continue receiving
                            tracing::warn!(
                                "Agent forwarder: lagged by {} messages for {}, continuing",
                                n,
                                ws_id
                            );
                            continue;
                        }
                    }
                }
            });
        }
    }
}
