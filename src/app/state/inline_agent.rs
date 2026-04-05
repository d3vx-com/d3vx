//! Inline agent types and parallel batch state

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Status of an inline spawned agent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineAgentStatus {
    Running,
    Completed,
    Ended,
    Failed,
    Cancelled,
}

/// Status of a child task within a parallel/multi-agent batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParallelChildStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Heuristic review/evaluation signals for a child candidate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateEvaluation {
    pub changed_file_quality: i32,
    pub test_lint_outcome: i32,
    pub docs_completeness: i32,
    pub conflict_risk: i32,
    pub scope_adherence: i32,
    pub total_score: i32,
    pub notes: Vec<String>,
}

/// Child task tracked under a coordinated multi-agent batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelChildTask {
    pub key: String,
    pub description: String,
    pub task: String,
    pub agent_type: String,
    pub specialist_role: String,
    pub depends_on: Vec<String>,
    pub ownership: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub status: ParallelChildStatus,
    pub result: Option<String>,
    pub evaluation: Option<CandidateEvaluation>,
    #[serde(default)]
    pub progress: u8,
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub blocker_reason: Option<String>,
    #[serde(default)]
    pub messages_sent: usize,
    #[serde(default)]
    pub messages_received: usize,
}

impl ParallelChildTask {
    pub fn new(key: String, specialist_role: String) -> Self {
        Self {
            key,
            description: String::new(),
            task: String::new(),
            agent_type: String::new(),
            specialist_role,
            depends_on: Vec::new(),
            ownership: None,
            task_id: None,
            agent_id: None,
            status: ParallelChildStatus::Pending,
            result: None,
            evaluation: None,
            progress: 0,
            blocked: false,
            blocker_reason: None,
            messages_sent: 0,
            messages_received: 0,
        }
    }

    pub fn set_blocked(&mut self, reason: String) {
        self.blocked = true;
        self.blocker_reason = Some(reason);
        self.status = ParallelChildStatus::Running;
    }

    pub fn clear_blocker(&mut self) {
        self.blocked = false;
        self.blocker_reason = None;
    }

    pub fn update_progress(&mut self, percent: u8) {
        self.progress = percent.min(100);
    }

    pub fn increment_sent(&mut self) {
        self.messages_sent += 1;
    }

    pub fn increment_received(&mut self) {
        self.messages_received += 1;
    }
}

/// Coordinated parent batch for a parallel multi-agent execution.
#[derive(Debug, Clone)]
pub struct ParallelBatchState {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub reasoning: String,
    pub select_best: bool,
    pub selection_criteria: Option<String>,
    pub selected_child_key: Option<String>,
    pub selection_reasoning: Option<String>,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
    pub children: Vec<ParallelChildTask>,
    pub coordination: super::super::handlers::agent::coordination::BatchCoordination,
    pub response_tx: std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
}

impl Default for ParallelBatchState {
    fn default() -> Self {
        Self {
            id: String::new(),
            parent_session_id: None,
            reasoning: String::new(),
            select_best: false,
            selection_criteria: None,
            selected_child_key: None,
            selection_reasoning: None,
            started_at: Instant::now(),
            completed_at: None,
            children: Vec::new(),
            coordination: Default::default(),
            response_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl Serialize for ParallelBatchState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ParallelBatchState", 10)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("parent_session_id", &self.parent_session_id)?;
        state.serialize_field("reasoning", &self.reasoning)?;
        state.serialize_field("select_best", &self.select_best)?;
        state.serialize_field("selection_criteria", &self.selection_criteria)?;
        state.serialize_field("selected_child_key", &self.selected_child_key)?;
        state.serialize_field("selection_reasoning", &self.selection_reasoning)?;
        state.serialize_field("children", &self.children)?;
        state.serialize_field("coordination", &self.coordination)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParallelBatchState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &'static [&'static str] = &[
            "id",
            "parent_session_id",
            "reasoning",
            "select_best",
            "selection_criteria",
            "selected_child_key",
            "selection_reasoning",
            "children",
            "coordination",
        ];
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Id,
            ParentSessionId,
            Reasoning,
            SelectBest,
            SelectionCriteria,
            SelectedChildKey,
            SelectionReasoning,
            Children,
            Coordination,
        }

        struct FieldVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = ParallelBatchState;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct ParallelBatchState")
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_string(value.to_owned())
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Str(&value),
                    &self,
                ))
            }
            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut parent_session_id = None;
                let mut reasoning = None;
                let mut select_best = None;
                let mut selection_criteria = None;
                let mut selected_child_key = None;
                let mut selection_reasoning = None;
                let mut children = None;
                let mut coordination = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            id = Some(map.next_value()?);
                        }
                        Field::ParentSessionId => {
                            parent_session_id = Some(map.next_value()?);
                        }
                        Field::Reasoning => {
                            reasoning = Some(map.next_value()?);
                        }
                        Field::SelectBest => {
                            select_best = Some(map.next_value()?);
                        }
                        Field::SelectionCriteria => {
                            selection_criteria = Some(map.next_value()?);
                        }
                        Field::SelectedChildKey => {
                            selected_child_key = Some(map.next_value()?);
                        }
                        Field::SelectionReasoning => {
                            selection_reasoning = Some(map.next_value()?);
                        }
                        Field::Children => {
                            children = Some(map.next_value()?);
                        }
                        Field::Coordination => {
                            coordination = Some(map.next_value()?);
                        }
                    }
                }
                Ok(ParallelBatchState {
                    id: id.unwrap_or_default(),
                    parent_session_id,
                    reasoning: reasoning.unwrap_or_default(),
                    select_best: select_best.unwrap_or(false),
                    selection_criteria,
                    selected_child_key,
                    selection_reasoning,
                    started_at: Instant::now(),
                    completed_at: None,
                    children: children.unwrap_or_default(),
                    coordination: coordination.unwrap_or_default(),
                    response_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
                })
            }
        }
        deserializer.deserialize_struct("ParallelBatchState", FIELDS, FieldVisitor)
    }
}

impl ParallelBatchState {
    #[allow(dead_code)]
    pub fn coordination_summary(
        &self,
    ) -> super::super::handlers::agent::coordination::CoordinationSummary {
        self.coordination.summary()
    }

    #[allow(dead_code)]
    pub fn get_child_mut(&mut self, key: &str) -> Option<&mut ParallelChildTask> {
        self.children.iter_mut().find(|c| c.key == key)
    }

    pub fn get_blocked_children(&self) -> Vec<&ParallelChildTask> {
        self.children.iter().filter(|c| c.blocked).collect()
    }

    pub fn has_blockers(&self) -> bool {
        self.coordination.has_blockers()
    }
}

impl ParallelBatchState {
    pub fn is_complete(&self) -> bool {
        self.children.iter().all(|child| {
            matches!(
                child.status,
                ParallelChildStatus::Completed
                    | ParallelChildStatus::Failed
                    | ParallelChildStatus::Cancelled
            )
        })
    }
}

/// Update operations for inline agents
#[derive(Debug, Clone)]
pub enum InlineAgentUpdate {
    Action(String),
    Tool(String),
    Output(String),
    Status(InlineAgentStatus),
    Message(AgentMessageLine),
}

impl InlineAgentUpdate {
    /// Apply this update to an inline agent
    pub fn apply(&self, agent: &mut InlineAgentInfo) {
        match self {
            InlineAgentUpdate::Action(action) => agent.set_action(action.clone()),
            InlineAgentUpdate::Tool(name) => agent.add_tool(name.clone()),
            InlineAgentUpdate::Output(line) => agent.add_output(line.clone()),
            InlineAgentUpdate::Status(status) => agent.status = *status,
            InlineAgentUpdate::Message(msg) => agent.add_message(msg.clone()),
        }
    }
}

/// Information about an inline spawned agent (no worktree, runs in-process)
#[derive(Debug, Clone)]
pub struct InlineAgentInfo {
    /// Unique ID for this agent
    pub id: String,
    /// Task description
    pub task: String,
    /// Status of the agent
    pub status: InlineAgentStatus,
    /// When the agent started
    pub start_time: Instant,
    /// Current action being performed
    pub current_action: Option<String>,
    /// Number of tools used so far
    pub tool_count: usize,
    /// Recent output lines (for expanded view)
    pub output_lines: Vec<String>,
    /// Whether this agent card is expanded
    pub expanded: bool,
    /// Whether to show tool calls and outputs in the expanded view
    pub show_tools: bool,
    /// Tool names used (for collapsed view)
    pub tools_used: Vec<String>,
    /// Full streaming messages (for expanded view)
    pub messages: Vec<AgentMessageLine>,
}

#[derive(Debug, Clone)]
pub struct AgentMessageLine {
    pub content: String,
    pub line_type: AgentLineType,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentLineType {
    Thinking,
    ToolCall,
    ToolOutput,
    Text,
}

impl InlineAgentInfo {
    /// Create a new inline agent info
    pub fn new(id: String, task: String) -> Self {
        Self {
            id,
            task,
            status: InlineAgentStatus::Running,
            start_time: Instant::now(),
            current_action: None,
            tool_count: 0,
            output_lines: Vec::new(),
            expanded: false,
            show_tools: false,
            tools_used: Vec::new(),
            messages: Vec::new(),
        }
    }

    /// Update the current action
    pub fn set_action(&mut self, action: String) {
        self.current_action = Some(action);
    }

    /// Increment tool count and track tool name
    pub fn add_tool(&mut self, tool_name: String) {
        self.tool_count += 1;
        if !self.tools_used.contains(&tool_name) {
            self.tools_used.push(tool_name);
        }
    }

    /// Add an output line
    pub fn add_output(&mut self, line: String) {
        self.output_lines.push(line.clone());
        if self.output_lines.len() > 50 {
            self.output_lines.remove(0);
        }
    }

    /// Add a message line
    pub fn add_message(&mut self, msg: AgentMessageLine) {
        self.messages.push(msg);
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    /// Get elapsed time as a formatted string
    pub fn elapsed(&self) -> String {
        let secs = self.start_time.elapsed().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{}m {}s", mins, secs)
        }
    }

    /// Get a short progress summary
    pub fn progress_summary(&self) -> String {
        match self.status {
            InlineAgentStatus::Running => {
                if let Some(ref action) = self.current_action {
                    let truncated = if action.len() > 40 {
                        format!("{}...", &action[..37])
                    } else {
                        action.clone()
                    };
                    format!("[{}] {}", self.elapsed(), truncated)
                } else {
                    format!("[{}] Running...", self.elapsed())
                }
            }
            InlineAgentStatus::Completed => format!("[Done in {}]", self.elapsed()),
            InlineAgentStatus::Ended => format!("[Ended in {}]", self.elapsed()),
            InlineAgentStatus::Failed => "[Failed]".to_string(),
            InlineAgentStatus::Cancelled => "[Cancelled]".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_agent_info_creation() {
        let agent =
            InlineAgentInfo::new("agent-1".to_string(), "Create a markdown file".to_string());
        assert_eq!(agent.id, "agent-1");
        assert_eq!(agent.task, "Create a markdown file");
        assert_eq!(agent.status, InlineAgentStatus::Running);
        assert!(!agent.expanded);
        assert!(!agent.show_tools);
        assert!(agent.tools_used.is_empty());
    }

    #[test]
    fn test_inline_agent_show_tools_toggle() {
        let mut agent = InlineAgentInfo::new("agent-2".to_string(), "Test task".to_string());
        assert!(!agent.show_tools);
        agent.show_tools = true;
        assert!(agent.show_tools);
        agent.show_tools = false;
        assert!(!agent.show_tools);
    }

    #[test]
    fn test_inline_agent_progress_summary_completed() {
        let mut agent = InlineAgentInfo::new("agent-5".to_string(), "Test task".to_string());
        agent.status = InlineAgentStatus::Completed;
        let summary = agent.progress_summary();
        assert!(summary.contains("Done"));
    }

    #[test]
    fn test_inline_agent_add_tool() {
        let mut agent = InlineAgentInfo::new("agent-3".to_string(), "Test task".to_string());
        assert_eq!(agent.tool_count, 0);

        agent.add_tool("Read".to_string());
        assert_eq!(agent.tool_count, 1);
        assert!(agent.tools_used.contains(&"Read".to_string()));

        agent.add_tool("Write".to_string());
        assert_eq!(agent.tool_count, 2);

        agent.add_tool("Read".to_string());
        assert_eq!(agent.tool_count, 3);
        assert_eq!(agent.tools_used.len(), 2);
    }

    #[test]
    fn test_inline_agent_progress_summary() {
        let agent = InlineAgentInfo::new("agent-4".to_string(), "Test task".to_string());
        let summary = agent.progress_summary();
        assert!(summary.contains("Running"));
    }
}
