//! REST API routes for the embedded web dashboard.
//!
//! Provides endpoints for listing sessions, sending messages, killing
//! sessions, merging PRs, and streaming SSE events. All handlers receive
//! a [`Dashboard`] via axum's State extractor.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info};

use super::sse;
use super::static_assets;
use super::types::{ApiResponse, SessionSummaryResponse};
use super::Dashboard;

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET / — serve the embedded dashboard HTML.
async fn index() -> impl IntoResponse {
    Html(static_assets::dashboard_html())
}

/// GET /api/sessions — list all sessions with lifecycle state.
async fn list_sessions(State(_dashboard): State<Dashboard>) -> impl IntoResponse {
    debug!("Listing sessions via API");
    // Placeholder: in production this queries the task store
    let sessions: Vec<SessionSummaryResponse> = Vec::new();
    Json(ApiResponse::ok(sessions))
}

/// GET /api/sessions/:id — session detail with cost, phase, PR.
async fn get_session(
    State(_dashboard): State<Dashboard>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    debug!("Get session detail: {}", session_id);
    // Placeholder: in production this queries the task store
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse {
            success: false,
            data: format!("Session {} not found", session_id),
        }),
    )
}

/// Request body for sending a message to a session.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

/// POST /api/sessions/:id/message — send a message to a running agent.
async fn send_message(
    State(dashboard): State<Dashboard>,
    Path(session_id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> impl IntoResponse {
    if body.message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<String> {
                success: false,
                data: "message field is required".to_string(),
            }),
        );
    }

    info!("Dashboard message to {}: {}", session_id, body.message);

    // Broadcast to SSE listeners; actual agent injection is via the
    // pipeline's agent nudge system.
    dashboard.broadcast(super::DashboardEvent::AgentActivity {
        task_id: session_id.clone(),
        state: format!("message: {}", body.message),
    });

    (
        StatusCode::OK,
        Json(ApiResponse::ok(format!("Message sent to {}", session_id))),
    )
}

/// POST /api/sessions/:id/kill — terminate a running session.
async fn kill_session(
    State(dashboard): State<Dashboard>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    info!("Dashboard kill request for {}", session_id);

    dashboard.broadcast(super::DashboardEvent::TaskStatusChanged {
        id: session_id.clone(),
        status: "killed".into(),
        phase: "cancelled".into(),
    });

    (
        StatusCode::OK,
        Json(ApiResponse::ok(format!(
            "Kill signal sent to {}",
            session_id
        ))),
    )
}

/// POST /api/prs/:id/merge — merge a PR.
async fn merge_pr(
    State(_dashboard): State<Dashboard>,
    Path(pr_id): Path<String>,
) -> impl IntoResponse {
    info!("Dashboard merge PR request: {}", pr_id);

    // Placeholder: in production this calls the PR lifecycle manager
    (
        StatusCode::OK,
        Json(ApiResponse::ok(format!("Merge requested for PR {}", pr_id))),
    )
}

// ---------------------------------------------------------------------------
// Router construction
// ---------------------------------------------------------------------------

/// Build the complete axum router with all dashboard routes.
pub fn create_router(dashboard: Dashboard) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(index))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/message", post(send_message))
        .route("/api/sessions/{id}/kill", post(kill_session))
        .route("/api/prs/{id}/merge", post(merge_pr))
        .route("/api/events", get(sse::events_stream))
        .with_state(dashboard)
        .layer(cors)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::dashboard::DashboardConfig;
    use axum::body::Body;
    use http::Request;
    use tower::ServiceExt;

    fn test_dashboard() -> Dashboard {
        Dashboard::new(DashboardConfig {
            enabled: true,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn test_index_returns_html() {
        let app = create_router(test_dashboard());
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_sessions_returns_empty() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .uri("/api/sessions")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_missing_session_returns_404() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .uri("/api/sessions/nonexistent")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_kill_session_returns_ok() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .method("POST")
            .uri("/api/sessions/T-99/kill")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_merge_pr_returns_ok() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .method("POST")
            .uri("/api/prs/42/merge")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_send_message_empty_returns_400() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .method("POST")
            .uri("/api/sessions/T-1/message")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":""}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_send_message_with_text() {
        let app = create_router(test_dashboard());
        let req = Request::builder()
            .method("POST")
            .uri("/api/sessions/T-1/message")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
