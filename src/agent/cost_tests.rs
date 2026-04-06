//! Cost calculator tests

use crate::agent::cost::{calculate_cost, get_pricing};
use crate::ipc::TokenUsage;

#[test]
fn test_pricing_claude_sonnet() {
    let pricing = get_pricing("claude-3-5-sonnet-20241022");
    assert_eq!(pricing.input, 3.0);
    assert_eq!(pricing.output, 15.0);
    assert!((pricing.cache_read - 0.30).abs() < f64::EPSILON);
}

#[test]
fn test_pricing_claude_opus() {
    let pricing = get_pricing("claude-3-opus-20240229");
    assert_eq!(pricing.input, 15.0);
    assert_eq!(pricing.output, 75.0);
}

#[test]
fn test_pricing_gpt4o() {
    let pricing = get_pricing("gpt-4o-2024-05-13");
    assert_eq!(pricing.input, 5.0);
    assert_eq!(pricing.output, 15.0);
}

#[test]
fn test_pricing_gpt35() {
    let pricing = get_pricing("gpt-3.5-turbo");
    assert_eq!(pricing.input, 0.15);
    assert_eq!(pricing.output, 0.60);
}

#[test]
fn test_pricing_default_fallback() {
    let pricing = get_pricing("some-unknown-model-name");
    assert_eq!(pricing.input, 3.0);
    assert_eq!(pricing.output, 15.0);
}

#[test]
fn test_pricing_case_insensitive() {
    let pricing = get_pricing("GPT-4O-MINI");
    assert_eq!(pricing.input, 0.15);
}

#[test]
fn test_calculate_cost_basic() {
    let usage = TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: None,
        total_cost: None,
    };
    let cost = calculate_cost(&usage, "claude-sonnet-4-20250514");
    // input: 1000/1M * 3.0 = 0.003
    // output: 500/1M * 15.0 = 0.0075
    assert!((cost - 0.0105).abs() < 0.0001);
}

#[test]
fn test_calculate_cost_with_cache() {
    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 0,
        cache_read_tokens: Some(500_000),
        total_cost: None,
    };
    let cost = calculate_cost(&usage, "claude-sonnet-4-20250514");
    // input: 1M/1M * 3.0 = 3.0
    // cache: 500K/1M * 0.30 = 0.15
    assert!((cost - 3.15).abs() < 0.01);
}

#[test]
fn test_calculate_cost_zero_tokens() {
    let usage = TokenUsage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: None,
        total_cost: None,
    };
    let cost = calculate_cost(&usage, "claude-sonnet-4-20250514");
    assert!((cost - 0.0).abs() < 0.0001);
}
