use serde::Serialize;

/// Dashboard API response types.

/// A task summary row displayed in the main table.
#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub title: String,
    pub state: String,
    pub phase: String,
    pub cost_usd: f64,
    pub duration_secs: u64,
    pub branch: Option<String>,
    pub agent_role: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

/// Detailed session/task information for the detail panel.
#[derive(Debug, Clone, Serialize)]
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

/// System stats shown in the dashboard header.
#[derive(Debug, Clone, Serialize)]
pub struct SystemStats {
    pub total_tasks: usize,
    pub active_tasks: usize,
    pub queued_tasks: usize,
    pub done_tasks: usize,
    pub failed_tasks: usize,
    pub cost_today: f64,
    pub queue_size: usize,
}

/// Cost breakdown by model.
#[derive(Debug, Clone, Serialize)]
pub struct ModelCost {
    pub model: String,
    pub cost_usd: f64,
    pub tokens: u64,
}

/// Budget info shown in the cost bar.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetInfo {
    pub spent_today_usd: f64,
    pub session_budget_usd: f64,
    pub daily_budget_usd: f64,
    pub warn_threshold: f64,
    pub paused: bool,
}

/// Generic API message envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    pub fn err(msg: &str) -> Self
    where
        T: Default,
    {
        Self {
            success: false,
            data: T::default(),
            error: Some(msg.to_string()),
        }
    }
}
