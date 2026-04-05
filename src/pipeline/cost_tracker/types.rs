//! Cost tracker types
//!
//! Defines the data types for API usage tracking and cost configuration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::super::phases::Phase;

/// Cost tracking for a single API call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsage {
    /// Number of input tokens
    pub input_tokens: u64,
    /// Number of output tokens
    pub output_tokens: u64,
    /// Cost in USD
    pub cost_usd: f64,
    /// Model used
    pub model: String,
    /// Phase where usage occurred
    pub phase: Phase,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ApiUsage {
    /// Create a new API usage record
    pub fn new(
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        model: String,
        phase: Phase,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cost_usd,
            model,
            phase,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Cost statistics for a task or session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostStats {
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Number of API calls
    pub api_calls: u64,
    /// Cost by phase
    pub cost_by_phase: HashMap<String, f64>,
    /// Tokens by model
    pub tokens_by_model: HashMap<String, (u64, u64)>, // (input, output)
}

impl CostStats {
    /// Merge another CostStats into this one
    pub fn merge(&mut self, other: &CostStats) {
        self.total_input_tokens += other.total_input_tokens;
        self.total_output_tokens += other.total_output_tokens;
        self.total_cost_usd += other.total_cost_usd;
        self.api_calls += other.api_calls;

        for (phase, cost) in &other.cost_by_phase {
            *self.cost_by_phase.entry(phase.clone()).or_insert(0.0) += cost;
        }

        for (model, (input, output)) in &other.tokens_by_model {
            let entry = self.tokens_by_model.entry(model.clone()).or_insert((0, 0));
            entry.0 += input;
            entry.1 += output;
        }
    }
}

/// Cost tracking configuration
#[derive(Debug, Clone)]
pub struct CostTrackerConfig {
    /// Maximum cost per task in USD (None = unlimited)
    pub max_task_cost: Option<f64>,
    /// Maximum cost per session in USD (None = unlimited)
    pub max_session_cost: Option<f64>,
    /// Enable detailed tracking by phase
    pub track_by_phase: bool,
    /// Enable model-specific tracking
    pub track_by_model: bool,
}

impl Default for CostTrackerConfig {
    fn default() -> Self {
        Self {
            max_task_cost: Some(10.0),     // $10 default limit
            max_session_cost: Some(100.0), // $100 default session limit
            track_by_phase: true,
            track_by_model: true,
        }
    }
}

/// Error types for cost tracking
#[derive(Debug, thiserror::Error)]
pub enum CostTrackerError {
    /// Budget exceeded
    #[error("Budget exceeded: ${actual:.2} > ${limit:.2}")]
    BudgetExceeded { actual: f64, limit: f64 },

    /// Invalid cost calculation
    #[error("Invalid cost calculation: {0}")]
    InvalidCalculation(String),
}

/// Helper function to estimate cost based on model and token counts
pub fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    // Cost per 1K tokens (as of 2024)
    let (input_cost_per_k, output_cost_per_k) = match model {
        // Anthropic Claude models
        m if m.contains("claude-3-opus") => (0.015, 0.075),
        m if m.contains("claude-3-sonnet") => (0.003, 0.015),
        m if m.contains("claude-3-haiku") => (0.00025, 0.00125),
        m if m.contains("claude-sonnet-4") => (0.003, 0.015),

        // OpenAI models
        m if m.contains("gpt-4-turbo") || m.contains("gpt-4-0125-preview") => (0.01, 0.03),
        m if m.contains("gpt-4") => (0.03, 0.06),
        m if m.contains("gpt-3.5-turbo") => (0.0005, 0.0015),
        m if m.contains("o1-preview") => (0.015, 0.06),
        m if m.contains("o1-mini") => (0.003, 0.012),

        // Google Gemini models
        m if m.contains("gemini-pro") => (0.00025, 0.0005),
        m if m.contains("gemini-ultra") => (0.0025, 0.0075),

        // Default (conservative estimate)
        _ => (0.01, 0.03),
    };

    let input_cost = (input_tokens as f64 / 1000.0) * input_cost_per_k;
    let output_cost = (output_tokens as f64 / 1000.0) * output_cost_per_k;

    input_cost + output_cost
}
