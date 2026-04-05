//! Agent loop lifecycle control: pause, resume, stop, step controller.

use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info};

use super::super::step_controller::{StepControl, StepController};
use super::types::AgentEvent;
use super::AgentLoop;

impl AgentLoop {
    /// Subscribe to agent events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Set the step controller for programmatic execution.
    pub async fn set_step_controller(&self, controller: StepController) {
        let mut slot = self.step_controller.lock().await;
        *slot = Some(controller);
    }

    pub(super) async fn append_step_controls(&self, controls: Vec<StepControl>) {
        if controls.is_empty() {
            return;
        }

        let mut slot = self.step_controller.lock().await;
        match slot.as_mut() {
            Some(controller) => controller.add_steps(controls),
            None => {
                let mut controller = StepController::new();
                controller.add_steps(controls);
                *slot = Some(controller);
            }
        }
    }

    pub(super) async fn next_program_step(&self) -> Option<StepControl> {
        let mut slot = self.step_controller.lock().await;
        let controller = slot.as_mut()?;
        let next = controller.next();
        let exhausted = next.is_none() && !controller.has_next();
        if exhausted {
            *slot = None;
        }
        next
    }

    /// Emit an event to subscribers.
    pub(super) fn emit(&self, event: AgentEvent) {
        if let Some(ref logger) = self.logger {
            let _ = logger.log(&event);
        }
        let _ = self.broadcast_tx.send(event);
    }

    /// Pause the agent loop.
    pub async fn pause(&self) {
        let mut paused = self.paused.write().await;
        *paused = true;
        let config = self.config.read().await;
        debug!(session_id = %config.session_id, "Agent paused");
    }

    /// Resume the agent loop.
    pub async fn resume(&self) {
        let mut paused = self.paused.write().await;
        *paused = false;
        let config = self.config.read().await;
        debug!(session_id = %config.session_id, "Agent resumed");
    }

    /// Check if the agent is paused.
    pub async fn is_paused(&self) -> bool {
        *self.paused.read().await
    }

    /// Stop the agent loop completely.
    pub async fn stop(&self) {
        let mut paused = self.paused.write().await;
        *paused = true;
        let config = self.config.read().await;
        info!(session_id = %config.session_id, "Agent stopped");
    }

    /// Wait if paused, return immediately otherwise.
    pub(super) async fn wait_if_paused(&self) {
        loop {
            let paused = *self.paused.read().await;
            if !paused {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
