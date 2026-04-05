//! Job Board Tools
//!
//! Six tools for agent task coordination: create, update, list, get, read output,
//! and halt jobs. Agents use these to coordinate work within a swarm or independently.

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

/// Lifecycle phase of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobPhase {
    Pending,
    Active,
    Done,
    Failed,
    Cancelled,
}

impl std::fmt::Display for JobPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Active => write!(f, "active"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A tracked work item on the job board.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEntry {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub phase: JobPhase,
    pub owner: Option<String>,
    pub blocked_by: Vec<String>,
    pub output: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl JobEntry {
    fn summary_json(&self) -> serde_json::Value {
        json!({
            "id": self.id,
            "title": self.title,
            "phase": self.phase.to_string(),
            "owner": self.owner,
            "blocked_by": self.blocked_by,
        })
    }
}

// -- Global registry --------------------------------------------------------

static JOB_BOARD: Lazy<RwLock<HashMap<String, JobEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn parse_phase(s: &str) -> Option<JobPhase> {
    match s {
        "pending" => Some(JobPhase::Pending),
        "active" => Some(JobPhase::Active),
        "done" => Some(JobPhase::Done),
        "failed" => Some(JobPhase::Failed),
        "cancelled" => Some(JobPhase::Cancelled),
        _ => None,
    }
}

// -- 1. CreateJobTool --------------------------------------------------------

#[derive(Clone, Default)]
pub struct CreateJobTool;

impl CreateJobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CreateJobTool {
    fn name(&self) -> String {
        "create_job".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Create a new tracked job on the shared job board. Returns the job ID."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Short title for the job" },
                    "description": { "type": "string", "description": "Optional details about the work" },
                    "blocked_by": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Job IDs that must complete before this one can start"
                    }
                },
                "required": ["title"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let title = match input.get("title").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return ToolResult::error("Missing required field: 'title'"),
        };
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);
        let blocked_by: Vec<String> = input
            .get("blocked_by")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let now = now_iso();
        let id = format!("job-{}", Uuid::new_v4().as_simple());
        let entry = JobEntry {
            id: id.clone(),
            title,
            description,
            phase: JobPhase::Pending,
            owner: None,
            blocked_by,
            output: None,
            created_at: now.clone(),
            updated_at: now,
        };

        debug!(job_id = %id, "created job");
        JOB_BOARD.write().unwrap().insert(id.clone(), entry);

        ToolResult::success(json!({ "id": id, "phase": "pending" }).to_string())
    }
}

// -- 2. UpdateJobTool --------------------------------------------------------

#[derive(Clone, Default)]
pub struct UpdateJobTool;

impl UpdateJobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for UpdateJobTool {
    fn name(&self) -> String {
        "update_job".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Update a job's phase, owner, or output. Use this to claim work, report progress, or mark completion.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" },
                    "phase": { "type": "string", "enum": ["pending","active","done","failed","cancelled"] },
                    "owner": { "type": "string", "description": "Call sign or agent ID claiming this job" },
                    "output": { "type": "string", "description": "Result or output to attach" }
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

        let mut board = JOB_BOARD.write().unwrap();
        let entry = match board.get_mut(&job_id) {
            Some(e) => e,
            None => return ToolResult::error(format!("Job '{}' not found", job_id)),
        };

        if let Some(phase_str) = input.get("phase").and_then(|v| v.as_str()) {
            match parse_phase(phase_str) {
                Some(p) => entry.phase = p,
                None => return ToolResult::error(format!("Invalid phase '{}'", phase_str)),
            }
        }
        if let Some(owner) = input.get("owner").and_then(|v| v.as_str()) {
            entry.owner = Some(owner.to_string());
        }
        if let Some(output) = input.get("output").and_then(|v| v.as_str()) {
            entry.output = Some(output.to_string());
        }
        entry.updated_at = now_iso();

        debug!(job_id = %job_id, phase = %entry.phase, "updated job");
        ToolResult::success(entry.summary_json().to_string())
    }
}

// -- 3. ListJobsTool ---------------------------------------------------------

#[derive(Clone, Default)]
pub struct ListJobsTool;

impl ListJobsTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ListJobsTool {
    fn name(&self) -> String {
        "list_jobs".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "List jobs on the board. Optionally filter by phase.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "phase": { "type": "string", "enum": ["pending","active","done","failed","cancelled"], "description": "Filter by phase (omit for all)" }
                }
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let filter = input
            .get("phase")
            .and_then(|v| v.as_str())
            .and_then(parse_phase);
        let board = JOB_BOARD.read().unwrap();
        let jobs: Vec<serde_json::Value> = board
            .values()
            .filter(|j| filter.map_or(true, |f| j.phase == f))
            .map(|j| j.summary_json())
            .collect();

        ToolResult::success(json!({ "count": jobs.len(), "jobs": jobs }).to_string())
    }
}

// -- 4. GetJobTool -----------------------------------------------------------

#[derive(Clone, Default)]
pub struct GetJobTool;

impl GetJobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GetJobTool {
    fn name(&self) -> String {
        "get_job".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Get full details for a single job by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" }
                },
                "required": ["job_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let job_id = match input.get("job_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return ToolResult::error("Missing required field: 'job_id'"),
        };

        let board = JOB_BOARD.read().unwrap();
        match board.get(job_id) {
            Some(entry) => ToolResult::success(json!(entry).to_string()),
            None => ToolResult::error(format!("Job '{}' not found", job_id)),
        }
    }
}

// -- 5. ReadJobOutputTool ----------------------------------------------------

#[derive(Clone, Default)]
pub struct ReadJobOutputTool;

impl ReadJobOutputTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadJobOutputTool {
    fn name(&self) -> String {
        "read_job_output".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Read the output attached to a completed job.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" }
                },
                "required": ["job_id"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let job_id = match input.get("job_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return ToolResult::error("Missing required field: 'job_id'"),
        };

        let board = JOB_BOARD.read().unwrap();
        match board.get(job_id) {
            Some(entry) => match &entry.output {
                Some(output) => ToolResult::success(output.clone()),
                None => ToolResult::error(format!("Job '{}' has no output yet", job_id)),
            },
            None => ToolResult::error(format!("Job '{}' not found", job_id)),
        }
    }
}

// -- 6. HaltJobTool ----------------------------------------------------------

#[derive(Clone, Default)]
pub struct HaltJobTool;

impl HaltJobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for HaltJobTool {
    fn name(&self) -> String {
        "halt_job".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Cancel a running or pending job.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string" }
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

        let mut board = JOB_BOARD.write().unwrap();
        match board.get_mut(&job_id) {
            Some(entry) => {
                if entry.phase == JobPhase::Done || entry.phase == JobPhase::Cancelled {
                    return ToolResult::error(format!(
                        "Job '{}' is already {}",
                        job_id, entry.phase
                    ));
                }
                entry.phase = JobPhase::Cancelled;
                entry.updated_at = now_iso();
                debug!(job_id = %job_id, "halted job");
                ToolResult::success(json!({ "id": job_id, "phase": "cancelled" }).to_string())
            }
            None => ToolResult::error(format!("Job '{}' not found", job_id)),
        }
    }
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_board() {
        JOB_BOARD.write().unwrap().clear();
    }

    fn make_ctx() -> ToolContext {
        ToolContext::default()
    }

    #[tokio::test]
    async fn create_job_returns_id() {
        clear_board();
        let tool = CreateJobTool::new();
        let result = tool
            .execute(
                json!({ "title": "Build API", "description": "Create REST endpoints" }),
                &make_ctx(),
            )
            .await;
        assert!(!result.is_error);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["phase"], "pending");
        assert!(body["id"].as_str().unwrap().starts_with("job-"));
    }

    #[tokio::test]
    async fn update_transitions_phase() {
        clear_board();
        let create = CreateJobTool::new();
        let created = create
            .execute(json!({ "title": "Test job" }), &make_ctx())
            .await;
        let id = serde_json::from_str::<serde_json::Value>(&created.content).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let update = UpdateJobTool::new();
        let result = update
            .execute(
                json!({ "job_id": id, "phase": "active", "owner": "agent-1" }),
                &make_ctx(),
            )
            .await;
        assert!(!result.is_error);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["phase"], "active");
        assert_eq!(body["owner"], "agent-1");
    }

    #[tokio::test]
    async fn list_filters_by_phase() {
        clear_board();
        let create = CreateJobTool::new();
        create.execute(json!({ "title": "A" }), &make_ctx()).await;
        create.execute(json!({ "title": "B" }), &make_ctx()).await;

        let list = ListJobsTool::new();
        let all = list.execute(json!({}), &make_ctx()).await;
        let body = serde_json::from_str::<serde_json::Value>(&all.content).unwrap();
        assert_eq!(body["count"], 2);

        let filtered = list.execute(json!({ "phase": "done" }), &make_ctx()).await;
        let body = serde_json::from_str::<serde_json::Value>(&filtered.content).unwrap();
        assert_eq!(body["count"], 0);
    }

    #[tokio::test]
    async fn get_and_read_output() {
        clear_board();
        let create = CreateJobTool::new();
        let created = create
            .execute(json!({ "title": "Output test" }), &make_ctx())
            .await;
        let id = serde_json::from_str::<serde_json::Value>(&created.content).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let update = UpdateJobTool::new();
        update
            .execute(
                json!({ "job_id": &id, "phase": "done", "output": "42 tests passed" }),
                &make_ctx(),
            )
            .await;

        let get = GetJobTool::new();
        let result = get.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(!result.is_error);

        let read = ReadJobOutputTool::new();
        let result = read.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(!result.is_error);
        assert!(result.content.contains("42 tests passed"));
    }

    #[tokio::test]
    async fn halt_cancels_active_job() {
        clear_board();
        let create = CreateJobTool::new();
        let created = create
            .execute(json!({ "title": "Halt test" }), &make_ctx())
            .await;
        let id = serde_json::from_str::<serde_json::Value>(&created.content).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let halt = HaltJobTool::new();
        let result = halt.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(!result.is_error);
        assert!(result.content.contains("cancelled"));

        let re_halt = halt.execute(json!({ "job_id": &id }), &make_ctx()).await;
        assert!(re_halt.is_error);
    }

    #[tokio::test]
    async fn errors_on_missing_job() {
        clear_board();
        let get = GetJobTool::new();
        let result = get
            .execute(json!({ "job_id": "job-nonexistent" }), &make_ctx())
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }
}
