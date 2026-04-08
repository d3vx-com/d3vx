//! Vex Agent Poller
//!
//! Polls for vex task events and maps them to InlineAgentInfo for display
//! in the TUI right panel alongside parallel agents.

use std::collections::HashSet;
use std::time::Instant;

use crate::app::state::{AgentLineType, AgentMessageLine, InlineAgentStatus};
use crate::app::InlineAgentInfo;
use crate::store::database::DatabaseHandle;
use crate::store::task::enums::ExecutionMode;
use crate::store::task::types::{Task, TaskLog};
use crate::store::TaskState;
use crate::store::TaskStore;

fn log_to_message_line(log: &TaskLog) -> Option<AgentMessageLine> {
    let data: serde_json::Value = serde_json::from_str(&log.data).ok()?;

    let (line_type, content) = match log.event.as_str() {
        "thinking" => {
            let text = data.get("text")?.as_str()?.to_string();
            (AgentLineType::Thinking, text)
        }
        "tool_start" => {
            let name = data.get("name")?.as_str()?.to_string();
            (AgentLineType::ToolCall, format!("Tool: {}", name))
        }
        "tool_end" => {
            let name = data.get("name")?.as_str()?.to_string();
            let is_error = data.get("is_error")?.as_bool().unwrap_or(false);
            if is_error {
                (AgentLineType::ToolOutput, format!("Tool {}: ERROR", name))
            } else {
                (AgentLineType::ToolOutput, format!("Tool {}: Done", name))
            }
        }
        "text" => {
            let text = data.get("text")?.as_str()?.to_string();
            let truncated = if text.len() > 200 {
                format!("{}...", &text[..197])
            } else {
                text
            };
            (AgentLineType::Text, truncated)
        }
        "error" => {
            let error = data.get("error")?.as_str()?.to_string();
            (AgentLineType::ToolOutput, format!("ERROR: {}", error))
        }
        _ => {
            let text = data
                .get("message")
                .or(data.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or(&log.event)
                .to_string();
            (AgentLineType::Text, text)
        }
    };

    Some(AgentMessageLine {
        content,
        line_type,
        timestamp: Instant::now(),
    })
}

fn logs_to_inline_agent(task: &Task, logs: Vec<TaskLog>) -> InlineAgentInfo {
    let mut agent = InlineAgentInfo::new(format!("vex:{}", task.id), task.title.clone());

    agent.status = match task.state {
        TaskState::Research | TaskState::Plan | TaskState::Implement | TaskState::Validate => {
            InlineAgentStatus::Running
        }
        TaskState::Done => InlineAgentStatus::Completed,
        TaskState::Failed => InlineAgentStatus::Failed,
        TaskState::Backlog | TaskState::Queued | TaskState::Preparing | TaskState::Spawning => {
            InlineAgentStatus::Running
        }
        _ => InlineAgentStatus::Running,
    };

    let mut tools_used = HashSet::new();
    for log in &logs {
        if let Some(line) = log_to_message_line(log) {
            agent.add_message(line);
        }

        if log.event == "tool_start" {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&log.data) {
                if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
                    tools_used.insert(name.to_string());
                }
            }
        }
    }

    agent.tools_used = tools_used.into_iter().collect();
    agent.tool_count = agent.tools_used.len();

    if let Some(last_log) = logs.last() {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&last_log.data) {
            if let Some(text) = data
                .get("text")
                .or(data.get("message"))
                .and_then(|v| v.as_str())
            {
                agent.current_action = Some(text.to_string());
            }
        }
    }

    agent.expanded = false;
    agent.show_tools = true;

    agent
}

pub fn poll_vex_agents(db: &DatabaseHandle, project_path: &str) -> Vec<InlineAgentInfo> {
    let db_lock = db.lock();
    let store = TaskStore::new(&*db_lock);

    let tasks = match store.list(crate::store::task::types::TaskListOptions {
        project_path: Some(project_path.to_string()),
        state: Some(vec![
            TaskState::Research,
            TaskState::Plan,
            TaskState::Implement,
            TaskState::Validate,
            TaskState::Preparing,
            TaskState::Spawning,
        ]),
        limit: Some(20),
        ..Default::default()
    }) {
        Ok(tasks) => tasks,
        Err(_) => return Vec::new(),
    };

    let vex_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|t| t.execution_mode == ExecutionMode::Vex)
        .collect();

    let mut agents = Vec::new();

    for task in vex_tasks {
        let logs = match store.get_logs(&task.id, None) {
            Ok(logs) => logs,
            Err(_) => continue,
        };

        let agent = logs_to_inline_agent(&task, logs);
        agents.push(agent);
    }

    agents
}

pub fn get_vex_agent(db: &DatabaseHandle, task_id: &str) -> Option<InlineAgentInfo> {
    let db_lock = db.lock();
    let store = TaskStore::new(&*db_lock);

    let task = store.get(task_id).ok().flatten()?;
    let logs = store.get_logs(task_id, None).ok()?;

    Some(logs_to_inline_agent(&task, logs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_to_message_line_thinking() {
        let log = TaskLog {
            id: 1,
            task_id: "test".to_string(),
            phase: "planning".to_string(),
            event: "thinking".to_string(),
            data: r#"{"text": "Analyzing the problem..."}"#.to_string(),
            duration_ms: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let line = log_to_message_line(&log);
        assert!(line.is_some());
        let line = line.unwrap();
        assert_eq!(line.line_type, AgentLineType::Thinking);
        assert!(line.content.contains("Analyzing"));
    }

    #[test]
    fn test_log_to_message_line_tool() {
        let log = TaskLog {
            id: 1,
            task_id: "test".to_string(),
            phase: "implementing".to_string(),
            event: "tool_start".to_string(),
            data: r#"{"name": "Read", "id": "tool-1"}"#.to_string(),
            duration_ms: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let line = log_to_message_line(&log);
        assert!(line.is_some());
        let line = line.unwrap();
        assert_eq!(line.line_type, AgentLineType::ToolCall);
        assert!(line.content.contains("Read"));
    }
}
