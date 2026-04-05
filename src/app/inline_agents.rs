//! Inline Agent Management
//!
//! Functions for managing inline (sub-) agent UI state.

use crate::app::state::{InlineAgentInfo, InlineAgentStatus, InlineAgentUpdate, RightPaneTab};
use crate::app::App;

impl App {
    /// Add a new inline agent to track
    pub fn add_inline_agent(&mut self, id: String, task: String) {
        self.agents
            .inline_agents
            .push(InlineAgentInfo::new(id, task));
    }

    /// Update an inline agent's status
    pub fn update_inline_agent(&mut self, id: &str, update: InlineAgentUpdate) {
        if let Some(agent) = self.agents.inline_agents.iter_mut().find(|a| a.id == id) {
            match update {
                InlineAgentUpdate::Action(action) => agent.set_action(action),
                InlineAgentUpdate::Tool(name) => agent.add_tool(name),
                InlineAgentUpdate::Output(line) => agent.add_output(line),
                InlineAgentUpdate::Status(status) => agent.status = status,
                InlineAgentUpdate::Message(msg) => agent.add_message(msg),
            }
        }
    }

    /// Toggle expand/collapse state of an inline agent
    pub fn toggle_inline_agent(&mut self, id: &str) {
        // First, check if the agent is already expanded
        let is_expanded = self
            .agents
            .inline_agents
            .iter()
            .any(|a| a.id == id && a.expanded);

        if is_expanded {
            // Already expanded - just collapse it
            if let Some(agent) = self.agents.inline_agents.iter_mut().find(|a| a.id == id) {
                agent.expanded = false;
            }
        } else {
            // Collapsed - expand it AND collapse all others
            for a in &mut self.agents.inline_agents {
                a.expanded = false;
            }
            if let Some(agent) = self.agents.inline_agents.iter_mut().find(|a| a.id == id) {
                agent.expanded = true;
            }
            self.ui.selected_agent_output_scroll = 0;
        }
    }

    /// Select an inline agent for focused inspection in the right pane.
    pub fn select_inline_agent(&mut self, agent_index: usize) {
        if agent_index >= self.agents.inline_agents.len() {
            return;
        }
        self.agents.selected_inline_agent = Some(agent_index);
        self.selected_right_pane_tab = RightPaneTab::Agent;
        let agent_id = self.agents.inline_agents[agent_index].id.clone();
        self.toggle_inline_agent(&agent_id);
    }

    /// Remove completed inline agents
    pub fn cleanup_inline_agents(&mut self) {
        self.agents
            .inline_agents
            .retain(|a| a.status == InlineAgentStatus::Running);
    }
}
