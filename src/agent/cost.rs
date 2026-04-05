//! Cost Calculator for LLM Usage
//!
//! Maps token usage to USD costs based on model pricing models.

use crate::ipc::TokenUsage;

/// Pricing for a specific model (per 1M tokens)
pub struct ModelPricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

/// Get pricing for a given model string
pub fn get_pricing(model: &str) -> ModelPricing {
    // 1. Try to get dynamic pricing from models.dev cache (OpenCode parity)
    if let Some(pricing) = crate::providers::pricing_cache::get_model_pricing(model) {
        return pricing;
    }

    // 2. Fallback to hardcoded estimates if the cache is missing or model not found
    match model.to_lowercase() {
        m if m.contains("claude-3-5-sonnet") => ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
        },
        m if m.contains("claude-3-opus") => ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_read: 0.0,
        },
        m if m.contains("gpt-4o") => ModelPricing {
            input: 5.0,
            output: 15.0,
            cache_read: 0.0,
        },
        m if m.contains("gpt-3.5") || m.contains("gpt-4-mini") => ModelPricing {
            input: 0.15,
            output: 0.60,
            cache_read: 0.0,
        },
        _ => ModelPricing {
            input: 3.0, // Default to Sonnet-like pricing
            output: 15.0,
            cache_read: 0.30,
        },
    }
}

/// Calculate USD cost for a token usage event
pub fn calculate_cost(usage: &TokenUsage, model: &str) -> f64 {
    let pricing = get_pricing(model);

    let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * pricing.output;
    let cache_cost = usage
        .cache_read_tokens
        .map(|t| (t as f64 / 1_000_000.0) * pricing.cache_read)
        .unwrap_or(0.0);

    input_cost + output_cost + cache_cost
}
