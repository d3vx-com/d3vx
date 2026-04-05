//! Agent state tracker implementation

use super::types::{is_valid_transition, AgentState, StateTransitionReason, DEFAULT_IDLE_TIMEOUT};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Tracks agent state with activity detection.
pub struct AgentStateTracker {
    /// Current state
    state: Arc<RwLock<AgentState>>,
    /// Last activity timestamp
    last_activity: Arc<RwLock<Instant>>,
    /// Idle timeout duration
    idle_timeout: Duration,
    /// Optional callback for state changes
    on_state_change:
        Option<Arc<dyn Fn(AgentState, AgentState, &StateTransitionReason) + Send + Sync>>,
    /// Number of iterations
    iterations: Arc<RwLock<u32>>,
}

impl AgentStateTracker {
    /// Create a new state tracker with default idle timeout.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(AgentState::Idle)),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            on_state_change: None,
            iterations: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a new state tracker with a custom idle timeout.
    pub fn with_idle_timeout(timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(AgentState::Idle)),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            idle_timeout: timeout,
            on_state_change: None,
            iterations: Arc::new(RwLock::new(0)),
        }
    }

    /// Set a callback for state changes.
    pub fn with_state_change_callback(
        mut self,
        callback: Arc<dyn Fn(AgentState, AgentState, &StateTransitionReason) + Send + Sync>,
    ) -> Self {
        self.on_state_change = Some(callback);
        self
    }

    /// Get the current state.
    pub async fn current_state(&self) -> AgentState {
        *self.state.read().await
    }

    /// Get the current number of iterations.
    pub async fn get_iterations(&self) -> u32 {
        *self.iterations.read().await
    }

    /// Get the last activity timestamp.
    pub async fn last_activity(&self) -> Instant {
        *self.last_activity.read().await
    }

    /// Check if the agent is currently active.
    pub async fn is_active(&self) -> bool {
        let s = self.current_state().await;
        s == AgentState::Thinking || s == AgentState::ToolExecution
    }

    /// Check if the agent is idle.
    pub async fn is_idle(&self) -> bool {
        *self.state.read().await == AgentState::Idle
    }

    /// Check if the agent is waiting for input.
    pub async fn is_waiting_input(&self) -> bool {
        *self.state.read().await == AgentState::WaitingInput
    }

    /// Check if the agent has exited.
    pub async fn is_exited(&self) -> bool {
        *self.state.read().await == AgentState::Done
    }

    /// Record activity (updates last_activity timestamp).
    /// Does not change state - use `transition_to` for state changes.
    pub async fn record_activity(&self) {
        let mut last = self.last_activity.write().await;
        *last = Instant::now();
    }

    /// Attempt to transition to a new state.
    /// Returns true if the transition was successful.
    pub async fn transition_to(
        &self,
        new_state: AgentState,
        reason: StateTransitionReason,
    ) -> bool {
        let current = self.current_state().await;

        // Check if transition is valid
        if !is_valid_transition(current, new_state) {
            debug!(
                from = ?current,
                to = ?new_state,
                reason = %reason,
                "Invalid state transition attempted"
            );
            return false;
        }

        // Skip if already in the target state
        if current == new_state {
            return true;
        }

        // Perform transition
        {
            let mut state = self.state.write().await;
            *state = new_state;
        }

        // Update activity timestamp for Active states
        if new_state == AgentState::Thinking || new_state == AgentState::ToolExecution {
            self.record_activity().await;
        }

        info!(
            from = ?current,
            to = ?new_state,
            reason = %reason,
            "Agent state transition"
        );

        // Call callback if set
        if let Some(ref callback) = self.on_state_change {
            callback(current, new_state, &reason);
        }

        true
    }

    /// Record activity and transition to Thinking if not already.
    pub async fn activate(&self, reason: StateTransitionReason) -> bool {
        self.transition_to(AgentState::Thinking, reason).await
    }

    /// Check for idle timeout and transition if necessary.
    /// Should be called periodically by the agent loop.
    pub async fn check_idle_timeout(&self) -> Option<AgentState> {
        let current = self.current_state().await;

        if current != AgentState::Thinking && current != AgentState::ToolExecution {
            return None;
        }

        let last = self.last_activity().await;
        let elapsed = last.elapsed();

        if elapsed >= self.idle_timeout {
            self.transition_to(AgentState::Idle, StateTransitionReason::IdleTimeout)
                .await;
            return Some(AgentState::Idle);
        }

        None
    }

    /// Mark that a tool requires user input.
    pub async fn request_input(&self, tool_name: &str) -> bool {
        self.transition_to(
            AgentState::WaitingInput,
            StateTransitionReason::ToolRequiresInput {
                tool_name: tool_name.to_string(),
            },
        )
        .await
    }

    /// Mark that user input has been received.
    pub async fn receive_input(&self) -> bool {
        self.transition_to(
            AgentState::Thinking,
            StateTransitionReason::UserInputReceived,
        )
        .await
    }

    /// Mark that the agent has completed.
    pub async fn complete(&self) -> bool {
        self.transition_to(
            AgentState::Done,
            StateTransitionReason::CompletedSuccessfully,
        )
        .await
    }

    /// Mark that the agent has failed.
    pub async fn fail(&self, error: &str) -> bool {
        self.transition_to(
            AgentState::Done,
            StateTransitionReason::Failed {
                error: error.to_string(),
            },
        )
        .await
    }

    /// Reset to idle state.
    pub async fn reset(&self) -> bool {
        let current = self.current_state().await;

        // Can only reset if not exited
        if current == AgentState::Done {
            debug!("Cannot reset from Exited state");
            return false;
        }

        {
            let mut state = self.state.write().await;
            *state = AgentState::Idle;
        }

        {
            let mut last = self.last_activity.write().await;
            *last = Instant::now();
        }

        if let Some(ref callback) = self.on_state_change {
            callback(current, AgentState::Idle, &StateTransitionReason::Reset);
        }

        true
    }

    /// Get time until idle timeout (if currently active).
    pub async fn time_until_idle(&self) -> Option<Duration> {
        let current = self.current_state().await;

        if current != AgentState::Thinking {
            return None;
        }

        let last = self.last_activity().await;
        let elapsed = last.elapsed();

        if elapsed >= self.idle_timeout {
            Some(Duration::ZERO)
        } else {
            Some(self.idle_timeout - elapsed)
        }
    }
}

impl Default for AgentStateTracker {
    fn default() -> Self {
        Self::new()
    }
}
