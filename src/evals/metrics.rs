//! Agent metrics reported back from an eval run.
//!
//! Kept in its own file so drivers that report rich metrics (cost,
//! iteration count, tool call count) don't pull the whole runner module
//! into scope just to fill in a result.

/// Metrics a driver reports after running a task.
///
/// Every field is optional — a driver that can't measure a metric sets
/// it to `None` so the runner doesn't have to invent a fake value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentMetrics {
    pub cost_usd: Option<f64>,
    pub iterations: Option<u32>,
    pub tool_calls: Option<u32>,
}

impl AgentMetrics {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = Some(cost_usd);
        self
    }

    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = Some(iterations);
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: u32) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }
}
