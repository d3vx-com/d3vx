//! Dashboard API response types.

use serde::Serialize;

/// Summary of a session returned by the REST API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSummaryResponse {
    pub session_id: String,
    pub phase: String,
    pub branch: Option<String>,
    pub cost_usd: f64,
    pub duration_secs: f64,
}

/// Detailed session information returned by the REST API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionDetailResponse {
    pub session_id: String,
    pub task_id: String,
    pub phase: String,
    pub branch: Option<String>,
    pub cost_usd: f64,
    pub duration_secs: f64,
    pub pr_url: Option<String>,
    pub message_count: usize,
}

/// Generic API message envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
        }
    }
}
