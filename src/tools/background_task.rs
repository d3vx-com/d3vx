//! Background Task Tools
//!
//! Two tools for managing background tasks:
//! - `TaskOutputTool`: Retrieve output from a background task, optionally blocking until completion.
//! - `TaskStopTool`: Stop a running background task.

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// -- Types ------------------------------------------------------------------

/// Status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundTaskStatus {
    Running,
    Done,
    Failed,
}

impl std::fmt::Display for BackgroundTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// A tracked background task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub command: String,
    pub status: BackgroundTaskStatus,
    pub output_path: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
}

impl BackgroundTask {
    fn summary_json(&self) -> serde_json::Value {
        json!({
            "id": self.id,
            "command": self.command,
            "status": self.status.to_string(),
            "output_path": self.output_path,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "exit_code": self.exit_code,
        })
    }
}

// -- Global registry --------------------------------------------------------

static BACKGROUND_TASKS: Lazy<RwLock<HashMap<String, BackgroundTask>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Register a new background task (used by external callers to seed the registry).
pub fn register_background_task(command: &str, output_path: Option<&str>) -> String {
    let id = format!("bg-{}", Uuid::new_v4().as_simple());
    let task = BackgroundTask {
        id: id.clone(),
        command: command.to_string(),
        status: BackgroundTaskStatus::Running,
        output_path: output_path.map(|p| p.to_string()),
        started_at: now_iso(),
        finished_at: None,
        exit_code: None,
    };
    BACKGROUND_TASKS.write().unwrap().insert(id.clone(), task);
    debug!(task_id = %id, "registered background task");
    id
}

/// Update a task to a completed state with an exit code.
pub fn complete_background_task(task_id: &str, exit_code: i32) {
    let mut registry = BACKGROUND_TASKS.write().unwrap();
    if let Some(task) = registry.get_mut(task_id) {
        task.status = if exit_code == 0 {
            BackgroundTaskStatus::Done
        } else {
            BackgroundTaskStatus::Failed
        };
        task.finished_at = Some(now_iso());
        task.exit_code = Some(exit_code);
        debug!(task_id = %task_id, exit_code, "completed background task");
    }
}

// -- 1. TaskOutputTool -------------------------------------------------------

#[derive(Clone, Default)]
pub struct TaskOutputTool;

impl TaskOutputTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> String {
        "task_output".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Retrieve output from a background task. When block=true (default), waits for the task to complete before returning output.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "ID of the background task to retrieve output for"
                    },
                    "block": {
                        "type": "boolean",
                        "description": "Whether to block until the task completes (default: true)",
                        "default": true
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time to wait in milliseconds when blocking (default: 30000)",
                        "default": 30000
                    }
                },
                "required": ["task_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let task_id = match input.get("task_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::error("Missing required field: 'task_id'"),
        };

        let block = input.get("block").and_then(|v| v.as_bool()).unwrap_or(true);

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);

        // Verify task exists
        {
            let registry = BACKGROUND_TASKS.read().unwrap();
            if !registry.contains_key(&task_id) {
                return ToolResult::error(format!("Background task '{}' not found", task_id));
            }
        }

        if block {
            let deadline =
                tokio::time::Instant::now() + tokio::time::Duration::from_millis(timeout_ms);
            loop {
                {
                    let registry = BACKGROUND_TASKS.read().unwrap();
                    if let Some(task) = registry.get(&task_id) {
                        if task.status != BackgroundTaskStatus::Running {
                            break;
                        }
                    } else {
                        return ToolResult::error(format!(
                            "Background task '{}' not found",
                            task_id
                        ));
                    }
                }

                if tokio::time::Instant::now() >= deadline {
                    return ToolResult::error(format!(
                        "Timed out waiting for background task '{}' after {}ms",
                        task_id, timeout_ms
                    ));
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }

        // Read final state
        let registry = BACKGROUND_TASKS.read().unwrap();
        match registry.get(&task_id) {
            Some(task) => {
                let output = match &task.output_path {
                    Some(path) => match std::fs::read_to_string(path) {
                        Ok(contents) => contents,
                        Err(e) => format!("(could not read output file: {})", e),
                    },
                    None => String::new(),
                };

                ToolResult::success(
                    json!({
                        "task": task.summary_json(),
                        "output": output,
                    })
                    .to_string(),
                )
            }
            None => ToolResult::error(format!("Background task '{}' not found", task_id)),
        }
    }
}

// -- 2. TaskStopTool ---------------------------------------------------------

#[derive(Clone, Default)]
pub struct TaskStopTool;

impl TaskStopTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> String {
        "task_stop".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description:
                "Stop a running background task by killing its process and marking it as failed."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "ID of the background task to stop"
                    }
                },
                "required": ["task_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let task_id = match input.get("task_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::error("Missing required field: 'task_id'"),
        };

        // Read task info, then drop the lock before async operations
        let command = {
            let registry = BACKGROUND_TASKS.read().unwrap();
            match registry.get(&task_id) {
                Some(task) => {
                    if task.status != BackgroundTaskStatus::Running {
                        return ToolResult::error(format!(
                            "Task '{}' is not running (status: {})",
                            task_id, task.status
                        ));
                    }
                    task.command.clone()
                }
                None => return ToolResult::error(format!("Task '{}' not found", task_id)),
            }
        }; // Lock dropped here

        // Attempt to kill the process — safe to await now
        let kill_result = tokio::process::Command::new("pkill")
            .arg("-f")
            .arg(&command)
            .output()
            .await;

        match kill_result {
            Ok(_) => debug!(task_id = %task_id, "sent kill signal for background task"),
            Err(e) => {
                debug!(task_id = %task_id, error = %e, "pkill failed, task may have already exited")
            }
        }

        // Re-acquire lock to update status
        let mut registry = BACKGROUND_TASKS.write().unwrap();
        if let Some(task) = registry.get_mut(&task_id) {
            task.status = BackgroundTaskStatus::Failed;
            task.finished_at = Some(now_iso());
            debug!(task_id = %task_id, "stopped background task");
            ToolResult::success(
                json!({
                    "stopped": true,
                    "task_id": task_id,
                    "status": "failed",
                })
                .to_string(),
            )
        } else {
            ToolResult::error(format!("Background task '{}' not found", task_id))
        }
    }
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn clear_registry() {
        BACKGROUND_TASKS.write().unwrap().clear();
    }

    fn make_ctx() -> ToolContext {
        ToolContext::default()
    }

    #[tokio::test]
    async fn task_output_missing_task_id() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let result = tool.execute(json!({}), &make_ctx()).await;
        assert!(result.is_error);
        assert!(result.content.contains("task_id"));
    }

    #[tokio::test]
    async fn task_output_task_not_found() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let result = tool
            .execute(json!({ "task_id": "nonexistent" }), &make_ctx())
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn task_output_non_blocking_returns_immediately() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let id = register_background_task("sleep 999", None);

        let result = tool
            .execute(json!({ "task_id": id, "block": false }), &make_ctx())
            .await;
        assert!(!result.is_error);

        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["task"]["status"], "running");
    }

    #[tokio::test]
    async fn task_output_blocking_reads_completed_task() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let id = register_background_task("echo done", None);
        // Manually complete the task so blocking resolves immediately.
        complete_background_task(&id, 0);

        let result = tool
            .execute(
                json!({ "task_id": id, "block": true, "timeout": 1000 }),
                &make_ctx(),
            )
            .await;
        assert!(!result.is_error);

        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["task"]["status"], "done");
        assert_eq!(body["task"]["exit_code"], 0);
    }

    #[tokio::test]
    async fn task_output_reads_output_file() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "hello from background").unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let id = register_background_task("echo hello", Some(&path));
        complete_background_task(&id, 0);

        let result = tool
            .execute(json!({ "task_id": id, "block": false }), &make_ctx())
            .await;
        assert!(!result.is_error);

        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["output"], "hello from background");
    }

    #[tokio::test]
    async fn task_output_blocking_times_out() {
        clear_registry();
        let tool = TaskOutputTool::new();

        // Register a task that stays running (never completed).
        let id = register_background_task("sleep 9999", None);

        let result = tool
            .execute(
                json!({ "task_id": &id, "block": true, "timeout": 200 }),
                &make_ctx(),
            )
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Timed out"));
    }

    #[tokio::test]
    async fn task_stop_missing_task_id() {
        clear_registry();
        let tool = TaskStopTool::new();

        let result = tool.execute(json!({}), &make_ctx()).await;
        assert!(result.is_error);
        assert!(result.content.contains("task_id"));
    }

    #[tokio::test]
    async fn task_stop_task_not_found() {
        clear_registry();
        let tool = TaskStopTool::new();

        let result = tool
            .execute(json!({ "task_id": "ghost" }), &make_ctx())
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn task_stop_marks_running_task_as_failed() {
        clear_registry();
        let tool = TaskStopTool::new();

        let id = register_background_task("sleep 999", None);

        let result = tool.execute(json!({ "task_id": &id }), &make_ctx()).await;
        assert!(!result.is_error);

        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["stopped"], true);
        assert_eq!(body["status"], "failed");

        // Verify registry state
        let registry = BACKGROUND_TASKS.read().unwrap();
        let task = registry.get(&id).unwrap();
        assert_eq!(task.status, BackgroundTaskStatus::Failed);
        assert!(task.finished_at.is_some());
    }

    #[tokio::test]
    async fn task_stop_already_finished_task() {
        clear_registry();
        let tool = TaskStopTool::new();

        let id = register_background_task("echo done", None);
        complete_background_task(&id, 0);

        let result = tool.execute(json!({ "task_id": &id }), &make_ctx()).await;
        assert!(result.is_error);
        assert!(result.content.contains("not running"));
    }

    #[tokio::test]
    async fn register_and_complete_lifecycle() {
        clear_registry();

        let id = register_background_task("echo lifecycle", None);
        {
            let registry = BACKGROUND_TASKS.read().unwrap();
            let task = registry.get(&id).unwrap();
            assert_eq!(task.status, BackgroundTaskStatus::Running);
            assert!(task.finished_at.is_none());
        }

        complete_background_task(&id, 1);
        {
            let registry = BACKGROUND_TASKS.read().unwrap();
            let task = registry.get(&id).unwrap();
            assert_eq!(task.status, BackgroundTaskStatus::Failed);
            assert_eq!(task.exit_code, Some(1));
            assert!(task.finished_at.is_some());
        }
    }

    #[tokio::test]
    async fn task_output_block_waits_then_resolves() {
        clear_registry();
        let tool = TaskOutputTool::new();

        let id = register_background_task("echo delayed", None);
        let id_clone = id.clone();

        // Spawn a task that completes the background task after a short delay.
        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            complete_background_task(&id_clone, 0);
        });

        let result = tool
            .execute(
                json!({ "task_id": &id, "block": true, "timeout": 2000 }),
                &make_ctx(),
            )
            .await;
        assert!(!result.is_error);

        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["task"]["status"], "done");

        handle.await.unwrap();
    }
}
