//! Agent State
//!
//! Encapsulates all agent-related state for the application.
//! This struct was extracted from the main App struct to reduce the God Object pattern.

use std::collections::HashMap;
use std::sync::Arc;

use crate::agent::{AgentEvent, AgentLoop};
use crate::app::state::{InlineAgentInfo, ParallelBatchState};

/// Agent-related state extracted from App
///
/// Contains all fields related to agent management, parallel execution,
/// and inline agent coordination. This allows agent state to be tested
/// in isolation and reduces coupling in the main App struct.
pub struct AgentState {
    // ════════════════════════════════════════════════════════════════════════════
    // Connection State
    // ════════════════════════════════════════════════════════════════════════════
    /// Whether connected to an agent
    pub is_connected: bool,

    // ════════════════════════════════════════════════════════════════════════════
    // Agent Loops
    // ════════════════════════════════════════════════════════════════════════════
    /// Agent loops mapped by workspace ID
    pub workspace_agents: HashMap<String, Arc<AgentLoop>>,
    /// Agent event receivers waiting to be spawned (move into background tasks)
    pub pending_agent_receivers: HashMap<String, tokio::sync::broadcast::Receiver<AgentEvent>>,
    /// Current active agent loop
    pub agent_loop: Option<Arc<AgentLoop>>,
    /// Current assistant message being streamed
    pub streaming_message: String,

    // ════════════════════════════════════════════════════════════════════════════
    // Inline Agents
    // ════════════════════════════════════════════════════════════════════════════
    /// Inline spawned agents (running in-process without worktrees)
    pub inline_agents: Vec<InlineAgentInfo>,
    /// Selected inline agent for keyboard navigation
    pub selected_inline_agent: Option<usize>,
    /// Whether parallel agent execution is enabled (for inline spawning)
    pub parallel_agents_enabled: bool,

    // ════════════════════════════════════════════════════════════════════════════
    // Parallel Execution
    // ════════════════════════════════════════════════════════════════════════════
    /// Coordinated parent-child parallel execution batches
    pub parallel_batches: HashMap<String, ParallelBatchState>,
    /// Receiver for SpawnParallel events (spawning parallel agents)
    pub spawn_parallel_receiver:
        Option<tokio::sync::mpsc::Receiver<crate::tools::SpawnParallelEvent>>,
    /// Queue of pending agent spawns (when concurrency limit is reached)
    pub pending_agent_queue: Vec<(String, crate::tools::SpawnTask)>,
    /// Current number of running parallel agents
    pub running_parallel_agents: usize,
    /// Number of active parallel batches (for consolidation triggering)
    pub active_parallel_batches: usize,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            is_connected: false,
            workspace_agents: HashMap::new(),
            pending_agent_receivers: HashMap::new(),
            agent_loop: None,
            streaming_message: String::new(),
            inline_agents: Vec::new(),
            selected_inline_agent: None,
            parallel_agents_enabled: true,
            parallel_batches: HashMap::new(),
            spawn_parallel_receiver: None,
            pending_agent_queue: Vec::new(),
            running_parallel_agents: 0,
            active_parallel_batches: 0,
        }
    }
}

impl AgentState {
    /// Create a new AgentState with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any agent is currently active
    pub fn has_active_agent(&self) -> bool {
        self.agent_loop.is_some() || !self.workspace_agents.is_empty()
    }

    /// Get the number of inline agents
    pub fn inline_agent_count(&self) -> usize {
        self.inline_agents.len()
    }

    /// Clear all agent state
    pub fn clear(&mut self) {
        self.is_connected = false;
        self.workspace_agents.clear();
        self.pending_agent_receivers.clear();
        self.agent_loop = None;
        self.streaming_message.clear();
        self.inline_agents.clear();
        self.selected_inline_agent = None;
        self.parallel_batches.clear();
        self.pending_agent_queue.clear();
        self.running_parallel_agents = 0;
        self.active_parallel_batches = 0;
    }

    /// Get parallel batches metadata as JSON for session serialization
    pub fn parallel_batches_metadata(&self) -> serde_json::Value {
        let batches: Vec<serde_json::Value> = self
            .parallel_batches
            .values()
            .map(|batch| {
                serde_json::json!({
                    "id": batch.id,
                    "parent_session_id": batch.parent_session_id,
                    "reasoning": batch.reasoning,
                    "select_best": batch.select_best,
                    "selection_criteria": batch.selection_criteria,
                    "selected_child_key": batch.selected_child_key,
                    "selection_reasoning": batch.selection_reasoning,
                    "coordination": {
                        "messages": batch.coordination.messages,
                        "synthesis_inputs": batch.coordination.synthesis_inputs,
                        "unresolved_blockers": batch.coordination.unresolved_blockers,
                        "last_progress_update": batch.coordination.last_progress_update,
                    },
                    "children": batch.children.iter().map(|child| serde_json::json!({
                        "key": child.key,
                        "description": child.description,
                        "task": child.task,
                        "agent_type": child.agent_type,
                        "specialist_role": child.specialist_role,
                        "depends_on": child.depends_on,
                        "ownership": child.ownership,
                        "task_id": child.task_id,
                        "agent_id": child.agent_id,
                        "status": format!("{:?}", child.status),
                        "result": child.result,
                        "evaluation": child.evaluation,
                        "progress": child.progress,
                        "blocked": child.blocked,
                        "blocker_reason": child.blocker_reason,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();

        serde_json::json!({
            "parallel_batches": batches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_default() {
        let state = AgentState::default();
        assert!(!state.is_connected);
        assert!(state.workspace_agents.is_empty());
        assert!(state.pending_agent_receivers.is_empty());
        assert!(state.agent_loop.is_none());
        assert!(state.streaming_message.is_empty());
        assert!(state.inline_agents.is_empty());
        assert!(state.parallel_batches.is_empty());
    }

    #[test]
    fn test_has_active_agent() {
        let mut state = AgentState::default();
        assert!(!state.has_active_agent());

        // Simulate having an agent loop
        state.agent_loop = None; // Can't easily create Arc<AgentLoop> in unit test
        assert!(!state.has_active_agent());
    }

    #[test]
    fn test_clear() {
        let mut state = AgentState::default();
        state.streaming_message = "test".to_string();
        state.is_connected = true;
        state.running_parallel_agents = 5;

        state.clear();

        assert!(state.streaming_message.is_empty());
        assert!(!state.is_connected);
        assert_eq!(state.running_parallel_agents, 0);
    }
}
