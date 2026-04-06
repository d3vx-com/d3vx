//! Cron Tools
//!
//! Three tools for scheduled task management: create, delete, and list cron jobs.
//! Agents use these to schedule recurring or one-shot prompts.

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

/// Status of a cron job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CronJobStatus {
    Active,
    Paused,
    Completed,
    Failed,
}

impl std::fmt::Display for CronJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// A scheduled cron entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronEntry {
    pub id: String,
    pub schedule: String,
    pub prompt: String,
    pub recurring: bool,
    pub status: CronJobStatus,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
}

impl CronEntry {
    fn summary_json(&self) -> serde_json::Value {
        json!({
            "id": self.id,
            "schedule": self.schedule,
            "prompt": self.prompt,
            "recurring": self.recurring,
            "status": self.status.to_string(),
            "last_run": self.last_run,
            "next_run": self.next_run,
        })
    }
}

// -- Global registry --------------------------------------------------------

static CRON_REGISTRY: Lazy<RwLock<HashMap<String, CronEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Validate that a cron expression has exactly 5 fields.
fn validate_cron_expr(expr: &str) -> Result<(), String> {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(format!(
            "Invalid cron expression '{}': expected 5 fields, got {}",
            expr,
            fields.len()
        ));
    }
    Ok(())
}

// -- 1. CronCreateTool -------------------------------------------------------

#[derive(Clone, Default)]
pub struct CronCreateTool;

impl CronCreateTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> String {
        "cron_create".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Create a scheduled cron job that enqueues a prompt on a recurring or one-shot basis."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "schedule": { "type": "string", "description": "5-field cron expression (min hour dom month dow)" },
                    "prompt": { "type": "string", "description": "Prompt to enqueue when the job fires" },
                    "recurring": { "type": "boolean", "description": "Whether to repeat (default true)" }
                },
                "required": ["schedule", "prompt"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let schedule = match input.get("schedule").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::error("Missing required field: 'schedule'"),
        };

        if let Err(e) = validate_cron_expr(&schedule) {
            return ToolResult::error(e);
        }

        let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::error("Missing required field: 'prompt'"),
        };

        let recurring = input
            .get("recurring")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let now = now_iso();
        let id = format!("cron-{}", Uuid::new_v4().as_simple());
        let entry = CronEntry {
            id: id.clone(),
            schedule,
            prompt,
            recurring,
            status: CronJobStatus::Active,
            last_run: None,
            next_run: None,
            created_at: now,
        };

        debug!(cron_id = %id, "created cron job");
        CRON_REGISTRY.write().unwrap().insert(id.clone(), entry);

        ToolResult::success(json!({ "id": id, "status": "active" }).to_string())
    }
}

// -- 2. CronDeleteTool -------------------------------------------------------

#[derive(Clone, Default)]
pub struct CronDeleteTool;

impl CronDeleteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> String {
        "cron_delete".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Delete a cron job by ID. Removes it from the registry permanently."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "ID of the cron job to delete" }
                },
                "required": ["job_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let job_id = match input.get("job_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::error("Missing required field: 'job_id'"),
        };

        let mut registry = CRON_REGISTRY.write().unwrap();
        match registry.remove(&job_id) {
            Some(removed) => {
                debug!(cron_id = %job_id, "deleted cron job");
                ToolResult::success(
                    json!({ "deleted": true, "id": job_id, "schedule": removed.schedule })
                        .to_string(),
                )
            }
            None => ToolResult::error(format!("Cron job '{}' not found", job_id)),
        }
    }
}

// -- 3. CronListTool ---------------------------------------------------------

#[derive(Clone, Default)]
pub struct CronListTool;

impl CronListTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> String {
        "cron_list".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "List all scheduled cron jobs.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let registry = CRON_REGISTRY.read().unwrap();
        let entries: Vec<serde_json::Value> = registry.values().map(|e| e.summary_json()).collect();

        ToolResult::success(json!({ "count": entries.len(), "jobs": entries }).to_string())
    }
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_registry() {
        CRON_REGISTRY.write().unwrap().clear();
    }

    fn make_ctx() -> ToolContext {
        ToolContext::default()
    }

    #[tokio::test]
    #[serial]
    async fn create_validates_cron_expression() {
        clear_registry();
        let tool = CronCreateTool::new();

        // Valid 5-field expression should succeed
        let result = tool
            .execute(
                json!({ "schedule": "0 * * * *", "prompt": "run tests" }),
                &make_ctx(),
            )
            .await;
        assert!(!result.is_error);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["status"], "active");
        assert!(body["id"].as_str().unwrap().starts_with("cron-"));

        // Invalid expression (too few fields) should fail
        let bad = tool
            .execute(
                json!({ "schedule": "0 * *", "prompt": "bad cron" }),
                &make_ctx(),
            )
            .await;
        assert!(bad.is_error);
        assert!(bad.content.contains("expected 5 fields"));

        // Missing schedule field
        let missing = tool
            .execute(json!({ "prompt": "no schedule" }), &make_ctx())
            .await;
        assert!(missing.is_error);
        assert!(missing.content.contains("schedule"));

        // Missing prompt field
        let no_prompt = tool
            .execute(json!({ "schedule": "0 0 * * *" }), &make_ctx())
            .await;
        assert!(no_prompt.is_error);
        assert!(no_prompt.content.contains("prompt"));
    }

    #[tokio::test]
    #[serial]
    async fn delete_removes_job() {
        clear_registry();
        let create = CronCreateTool::new();
        let created = create
            .execute(
                json!({ "schedule": "0 0 * * *", "prompt": "daily check" }),
                &make_ctx(),
            )
            .await;
        let id = serde_json::from_str::<serde_json::Value>(&created.content).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let delete = CronDeleteTool::new();
        let result = delete.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(!result.is_error);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["deleted"], true);
        assert_eq!(body["id"], id);

        // Deleting again should fail
        let again = delete.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(again.is_error);
        assert!(again.content.contains("not found"));
    }

    #[tokio::test]
    #[serial]
    async fn list_returns_all_entries() {
        clear_registry();
        let create = CronCreateTool::new();
        create
            .execute(
                json!({ "schedule": "*/5 * * * *", "prompt": "check health", "recurring": true }),
                &make_ctx(),
            )
            .await;
        create
            .execute(
                json!({ "schedule": "0 8 * * 1", "prompt": "weekly report", "recurring": false }),
                &make_ctx(),
            )
            .await;

        let list = CronListTool::new();
        let result = list.execute(json!({}), &make_ctx()).await;
        assert!(!result.is_error);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["count"], 2);

        let jobs = body["jobs"].as_array().unwrap();
        assert_eq!(jobs.len(), 2);
    }
}
