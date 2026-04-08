//! REST API routes for the embedded web dashboard.
//!
//! Endpoints query the SQLite store directly for live session/task data.
//! Static assets (React SPA) are served from compiled-in files.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::{info, warn};

use super::sse;
use super::static_assets;
use super::types::{BudgetInfo, ModelCost, SessionDetailResponse, SystemStats, TaskRow};
use super::Dashboard;

// ---------------------------------------------------------------------------
// Response helpers — all return concrete `Response` to avoid impl Trait
// opaque-type conflicts.
// ---------------------------------------------------------------------------

fn json_ok<T: serde::Serialize>(data: T) -> Response {
    Json(serde_json::json!({ "success": true, "data": data })).into_response()
}

fn json_err(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "success": false, "error": msg })),
    )
        .into_response()
}

fn json_not_found(msg: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "success": false, "error": msg })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /api/tasks — list all tasks as a flat table.
async fn list_tasks(State(dashboard): State<Dashboard>) -> Response {
    let db = dashboard.db();
    let db_lock = db.lock();

    let task_store = crate::store::TaskStore::new(&db_lock);
    let tasks = task_store.list(crate::store::TaskListOptions {
        limit: Some(200),
        ..Default::default()
    });

    let rows: Vec<TaskRow> = tasks.map_or_else(
        |e| {
            warn!("Failed to query tasks: {}", e);
            Vec::new()
        },
        map_tasks_to_rows,
    );

    json_ok(rows)
}

/// GET /api/tasks/:id — get detailed task info.
async fn get_task(State(dashboard): State<Dashboard>, Path(task_id): Path<String>) -> Response {
    let db = dashboard.db();
    let db_lock = db.lock();

    let task_store = crate::store::TaskStore::new(&db_lock);
    let task = match task_store.get(&task_id) {
        Ok(Some(t)) => t,
        Ok(None) => return json_not_found("Task not found"),
        Err(e) => {
            warn!("Query error: {}", e);
            return json_not_found("Database error");
        }
    };

    let session_store = crate::store::SessionStore::new(&db_lock);
    let sessions = session_store
        .list(crate::store::SessionListOptions {
            task_id: Some(task_id.clone()),
            ..Default::default()
        })
        .unwrap_or_default();

    let cost_usd = extract_cost_usd(&task.metadata).unwrap_or(0.0);
    let duration_secs = match (&task.started_at, &task.completed_at) {
        (Some(start), Some(end)) => parse_duration_secs(start, end),
        (Some(start), None) => parse_duration_secs_since(start),
        _ => 0,
    };

    json_ok(SessionDetailResponse {
        session_id: task.id,
        task_id,
        phase: task.pipeline_phase.unwrap_or_default(),
        branch: task.worktree_branch,
        cost_usd,
        duration_secs: duration_secs as f64,
        pr_url: None,
        message_count: sessions.len(),
    })
}

/// GET /api/stats — system-level stats.
async fn get_stats(State(dashboard): State<Dashboard>) -> Response {
    let db = dashboard.db();
    let db_lock = db.lock();

    let task_store = crate::store::TaskStore::new(&db_lock);
    let counts = task_store.get_counts().unwrap_or_default();

    use crate::store::task::state_machine::TaskState;
    let active_states = [
        TaskState::Spawning,
        TaskState::Queued,
        TaskState::Research,
        TaskState::Plan,
        TaskState::Implement,
        TaskState::Validate,
        TaskState::Review,
        TaskState::Test,
        TaskState::Fix,
        TaskState::Investigate,
        TaskState::Analyze,
        TaskState::Harden,
        TaskState::Cleanup,
        TaskState::Execute,
        TaskState::Prepare,
        TaskState::Preparing,
    ];
    let active: usize = active_states
        .iter()
        .filter_map(|s| counts.get(s))
        .map(|c| *c as usize)
        .sum();

    let queued = counts.get(&TaskState::Queued).copied().unwrap_or(0) as usize;
    let done = counts.get(&TaskState::Done).copied().unwrap_or(0) as usize;
    let failed = counts.get(&TaskState::Failed).copied().unwrap_or(0) as usize;
    let total: usize = counts.values().map(|v| *v as usize).sum();

    json_ok(SystemStats {
        total_tasks: total,
        active_tasks: active,
        queued_tasks: queued,
        done_tasks: done,
        failed_tasks: failed,
        cost_today: 0.0,
        queue_size: queued,
    })
}

/// GET /api/costs — cost breakdown by model.
async fn get_costs(State(dashboard): State<Dashboard>) -> Response {
    let db = dashboard.db();
    let db_lock = db.lock();

    let session_store = crate::store::SessionStore::new(&db_lock);
    let all_sessions = session_store
        .list(crate::store::SessionListOptions::default())
        .unwrap_or_default();

    let mut by_model: HashMap<String, (f64, u64)> = HashMap::new();
    let mut total: f64 = 0.0;

    for s in &all_sessions {
        let cost = extract_cost_usd(&s.metadata).unwrap_or(0.0);
        let tokens = s.token_count as u64;
        total += cost;
        let entry = by_model.entry(s.model.clone()).or_default();
        entry.0 += cost;
        entry.1 += tokens;
    }

    let model_costs: Vec<ModelCost> = by_model
        .into_iter()
        .map(|(model, (cost_usd, tokens))| ModelCost {
            model,
            cost_usd,
            tokens,
        })
        .collect();

    let budget = BudgetInfo {
        spent_today_usd: total,
        session_budget_usd: 5.0,
        daily_budget_usd: 50.0,
        warn_threshold: 0.8,
        paused: false,
    };

    json_ok(serde_json::json!([model_costs, budget]))
}

/// GET /api/budget — budget enforcement config and current state.
async fn get_budget(State(_dashboard): State<Dashboard>) -> Response {
    json_ok(BudgetInfo {
        spent_today_usd: 0.0,
        session_budget_usd: 5.0,
        daily_budget_usd: 50.0,
        warn_threshold: 0.8,
        paused: false,
    })
}

/// POST /api/tasks/:id/message — send a message to a running agent.
async fn send_message(
    State(dashboard): State<Dashboard>,
    Path(task_id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Response {
    if body.message.is_empty() {
        return json_err("message field is required");
    }

    info!("Dashboard message to {}: {}", task_id, body.message);
    dashboard.broadcast(super::DashboardEvent::AgentActivity {
        task_id: task_id.clone(),
        state: format!("U+{}: {}", task_id, body.message),
    });

    json_ok(format!("Message sent to {}", task_id))
}

/// POST /api/tasks/:id/kill — terminate a running task.
async fn kill_task(State(dashboard): State<Dashboard>, Path(task_id): Path<String>) -> Response {
    info!("Dashboard kill request for {}", task_id);

    {
        let db = dashboard.db();
        let db_lock = db.lock();
        let task_store = crate::store::TaskStore::new(&db_lock);
        match task_store.get(&task_id) {
            Ok(Some(_)) => {
                if let Err(e) = task_store.transition(
                    &task_id,
                    crate::store::task::state_machine::TaskState::Failed,
                ) {
                    warn!("Failed to transition task {}: {}", task_id, e);
                }
            }
            Ok(None) => warn!("Kill requested for non-existent task: {}", task_id),
            Err(e) => warn!("Database error on kill: {}", e),
        }
    }

    dashboard.broadcast(super::DashboardEvent::TaskStatusChanged {
        id: task_id.clone(),
        status: "killed".into(),
        phase: "cancelled".into(),
    });

    json_ok(format!("Kill signal sent to {}", task_id))
}

/// POST /api/tasks/:id/retry — retry a failed task.
async fn retry_task(State(dashboard): State<Dashboard>, Path(task_id): Path<String>) -> Response {
    info!("Dashboard retry request for {}", task_id);

    let db = dashboard.db();
    let db_lock = db.lock();
    let task_store = crate::store::TaskStore::new(&db_lock);

    match task_store.retry(&task_id) {
        Ok(true) => json_ok(format!("Task {} queued for retry", task_id)),
        Ok(false) => json_err("Max retries reached or task not found"),
        Err(e) => {
            warn!("Retry error: {}", e);
            json_err("Retry failed")
        }
    }
}

// ---------------------------------------------------------------------------
// Request body
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn create_router(dashboard: Dashboard) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}/message", post(send_message))
        .route("/tasks/{id}/kill", post(kill_task))
        .route("/tasks/{id}/retry", post(retry_task))
        .route("/stats", get(get_stats))
        .route("/costs", get(get_costs))
        .route("/budget", get(get_budget))
        .route("/events", get(sse::events_stream));

    let static_dir = ServeDir::new(static_assets::static_dir());

    Router::new()
        .nest("/api", api)
        .fallback_service(static_dir)
        .layer(cors)
        .with_state(dashboard)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn map_tasks_to_rows(tasks: Vec<crate::store::Task>) -> Vec<TaskRow> {
    tasks
        .into_iter()
        .map(|t| TaskRow {
            id: t.id,
            title: t.title,
            state: t.state.to_string(),
            phase: t.pipeline_phase.unwrap_or_default(),
            cost_usd: extract_cost_usd(&t.metadata).unwrap_or(0.0),
            duration_secs: match (&t.started_at, &t.completed_at) {
                (Some(start), Some(end)) => parse_duration_secs(start, end),
                (Some(start), None) => parse_duration_secs_since(start),
                _ => 0,
            },
            branch: t.worktree_branch.clone(),
            agent_role: t.agent_role.map(|r| r.to_string()),
            error: t.error,
            created_at: t.created_at,
        })
        .collect()
}

fn extract_cost_usd(json: &str) -> Option<f64> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|m| m.get("cost_usd").and_then(|v| v.as_f64()))
}

fn parse_duration_secs(start: &str, end: &str) -> u64 {
    let Ok(s) = chrono::DateTime::parse_from_rfc3339(start) else {
        return 0;
    };
    let Ok(e) = chrono::DateTime::parse_from_rfc3339(end) else {
        return 0;
    };
    (e.timestamp() - s.timestamp()).max(0) as u64
}

fn parse_duration_secs_since(start: &str) -> u64 {
    let Ok(s) = chrono::DateTime::parse_from_rfc3339(start) else {
        return 0;
    };
    (chrono::Utc::now().timestamp() - s.timestamp()).max(0) as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::dashboard::DashboardConfig;
    use crate::store::Database;
    use axum::body::Body;
    use http::Request;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_dashboard() -> Dashboard {
        Dashboard::new(
            DashboardConfig {
                enabled: true,
                ..Default::default()
            },
            Arc::new(parking_lot::Mutex::new(Database::in_memory().unwrap())),
        )
    }

    #[tokio::test]
    async fn test_list_returns_ok() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_stats_returns_ok() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_missing_task_404() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/tasks/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_send_empty_400() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/T-1/message")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"message":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_send_with_text() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/T-1/message")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"message":"hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_budget_ok() {
        let app = create_router(test_dashboard());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/budget")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
