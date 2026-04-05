//! Coordinator Prompt Builder
//!
//! Constructs system prompts and decision prompts for the coordinator
//! meta-agent, including available tools, project state, and guidelines.

use super::types::CoordinatorState;

// ---------------------------------------------------------------------------
// Prompt builder
// ---------------------------------------------------------------------------

/// Builds prompts for the coordinator meta-agent.
pub struct CoordinatorPromptBuilder;

impl CoordinatorPromptBuilder {
    /// Build the system prompt that sets up the coordinator's role.
    ///
    /// The prompt includes:
    /// - Role description
    /// - Available tools and descriptions
    /// - Current project state
    /// - Decision guidelines
    pub fn build_system_prompt(state: &CoordinatorState, project: &str) -> String {
        let status = Self::build_status_overview(state);
        format!(
            r#"You are the d3vx coordinator meta-agent for project "{project}".

Your role is to manage multiple concurrent agent sessions, ensuring efficient
task execution and timely intervention when sessions stall.

## Available Tools

1. **LaunchAgent** -- Spawn a new agent session with a prompt.
2. **ListSessions** -- Return all active session statuses.
3. **SendNudge** -- Send a message to a running agent session.
4. **KillSession** -- Terminate a stuck or unwanted session.
5. **GetStatus** -- Get detailed status for one session.
6. **BatchLaunch** -- Launch multiple issues in parallel.

## Current State

{status}

## Decision Guidelines

- **Spawn** new agents when there is backlog and capacity available.
- **Nudge** agents that appear to be idle or off-track but not yet stuck.
- **Kill** sessions that have been stuck for an extended period with no
  progress, after at least one nudge attempt.
- **Batch launch** when multiple independent issues are ready.
- Do NOT exceed the configured maximum parallel sessions.
- Always provide a clear rationale for every decision.

Respond with a JSON action describing your next move."#
        )
    }

    /// Build a compact status overview from the coordinator state.
    pub fn build_status_overview(state: &CoordinatorState) -> String {
        let active = state.active_sessions.len();
        let pending = state.pending_reviews.len();
        let stuck = state.stuck_sessions.len();

        let mut lines = vec![format!(
            "Active: {} | Pending reviews: {} | Stuck: {}",
            active, pending, stuck
        )];

        if !state.active_sessions.is_empty() {
            lines.push("Active sessions:".to_string());
            for id in &state.active_sessions {
                lines.push(format!("  - {}", id));
            }
        }

        if !state.pending_reviews.is_empty() {
            lines.push("Pending reviews:".to_string());
            for id in &state.pending_reviews {
                lines.push(format!("  - {}", id));
            }
        }

        if !state.stuck_sessions.is_empty() {
            lines.push("Stuck sessions (consider nudging or killing):".to_string());
            for id in &state.stuck_sessions {
                lines.push(format!("  - {}", id));
            }
        }

        lines.join("\n")
    }

    /// Build a decision prompt for a specific event.
    pub fn build_decision_prompt(event: &str, state: &CoordinatorState) -> String {
        let status = Self::build_status_overview(state);
        format!(
            r#"Event: {event}

Current state:
{status}

What action should the coordinator take? Respond with a JSON object containing:
- "tool": the tool name and parameters
- "rationale": why you chose this action"#
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> CoordinatorState {
        CoordinatorState {
            active_sessions: vec!["sess-1".to_string(), "sess-2".to_string()],
            pending_reviews: vec!["sess-3".to_string()],
            stuck_sessions: vec!["sess-4".to_string()],
        }
    }

    #[test]
    fn test_prompt_builder_includes_tools() {
        let state = CoordinatorState::default();
        let prompt = CoordinatorPromptBuilder::build_system_prompt(&state, "my-project");

        assert!(
            prompt.contains("LaunchAgent"),
            "should mention LaunchAgent tool"
        );
        assert!(
            prompt.contains("ListSessions"),
            "should mention ListSessions tool"
        );
        assert!(
            prompt.contains("SendNudge"),
            "should mention SendNudge tool"
        );
        assert!(
            prompt.contains("KillSession"),
            "should mention KillSession tool"
        );
        assert!(
            prompt.contains("GetStatus"),
            "should mention GetStatus tool"
        );
        assert!(
            prompt.contains("BatchLaunch"),
            "should mention BatchLaunch tool"
        );
    }

    #[test]
    fn test_prompt_builder_includes_state() {
        let state = sample_state();
        let prompt = CoordinatorPromptBuilder::build_system_prompt(&state, "my-project");

        assert!(
            prompt.contains("sess-1"),
            "should include active session IDs"
        );
        assert!(
            prompt.contains("sess-3"),
            "should include pending review IDs"
        );
        assert!(
            prompt.contains("sess-4"),
            "should include stuck session IDs"
        );
        assert!(prompt.contains("my-project"), "should include project name");
    }

    #[test]
    fn test_prompt_builder_includes_guidelines() {
        let state = CoordinatorState::default();
        let prompt = CoordinatorPromptBuilder::build_system_prompt(&state, "proj");

        assert!(
            prompt.contains("Decision Guidelines"),
            "should include guidelines section"
        );
        assert!(prompt.contains("Spawn"), "should mention spawning");
        assert!(prompt.contains("Nudge"), "should mention nudging");
        assert!(prompt.contains("Kill"), "should mention killing");
    }

    #[test]
    fn test_status_overview_counts() {
        let state = sample_state();
        let overview = CoordinatorPromptBuilder::build_status_overview(&state);

        assert!(overview.contains("Active: 2"));
        assert!(overview.contains("Pending reviews: 1"));
        assert!(overview.contains("Stuck: 1"));
    }

    #[test]
    fn test_status_overview_empty_state() {
        let state = CoordinatorState::default();
        let overview = CoordinatorPromptBuilder::build_status_overview(&state);

        assert!(overview.contains("Active: 0"));
        assert!(overview.contains("Pending reviews: 0"));
        assert!(overview.contains("Stuck: 0"));
    }

    #[test]
    fn test_decision_prompt_includes_event() {
        let state = CoordinatorState::default();
        let prompt =
            CoordinatorPromptBuilder::build_decision_prompt("session sess-1 timed out", &state);

        assert!(prompt.contains("session sess-1 timed out"));
        assert!(prompt.contains("What action should"));
    }

    #[test]
    fn test_decision_prompt_includes_current_state() {
        let state = sample_state();
        let prompt = CoordinatorPromptBuilder::build_decision_prompt("new issue opened", &state);

        assert!(prompt.contains("sess-1"));
        assert!(prompt.contains("sess-4"));
    }
}
