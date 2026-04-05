//! Token estimation tests

use super::*;

// ── Estimator Tests ──────────────────────────────────────────

#[test]
fn test_text_empty() {
    assert_eq!(estimate_tokens_for_text(""), 0);
}

#[test]
fn test_text_short_input() {
    assert_eq!(estimate_tokens_for_text("a"), 0);
    assert_eq!(estimate_tokens_for_text("ab"), 0);
    assert_eq!(estimate_tokens_for_text("abc"), 0); // 3 chars < 4
}

#[test]
fn test_text_boundary() {
    assert_eq!(estimate_tokens_for_text("abcd"), 1);
    assert_eq!(estimate_tokens_for_text("abcde"), 1);
    assert_eq!(estimate_tokens_for_text("abcdefgh"), 2);
}

#[test]
fn test_text_multibyte_bytes() {
    // "你好世界" = 4 × 3 bytes in UTF-8 = 12 bytes → 3 tokens
    assert_eq!(estimate_tokens_for_text("你好世界"), 3);
}

#[test]
fn test_text_realistic_length() {
    assert!(estimate_tokens_for_text("Hello, this is a test sentence.") > 0);
}

#[test]
fn test_content_estimation_passthrough() {
    assert_eq!(estimate_for_content("abcd"), 1);
    assert_eq!(estimate_for_content(""), 0);
}

#[test]
fn test_code_estimation() {
    // Code uses chars/token × 1.2, so fewer tokens
    let code = "fn main() { println!(\"Hello\"); }";
    let text_tokens = estimate_tokens_for_text(code);
    let code_tokens = estimate_for_code(code);
    assert!(code_tokens <= text_tokens, "code tokens ({code_tokens}) should be <= text tokens ({text_tokens})");
    assert!(code_tokens > 0);
}

#[test]
fn test_code_estimation_empty() {
    assert_eq!(estimate_for_code(""), 0);
}

#[test]
fn test_json_estimation() {
    // JSON uses chars/token × 0.9, so more tokens (repetitive structure)
    let json = r#"{"key": "value", "a": 1}"#;
    let text_tokens = estimate_tokens_for_text(json);
    let json_tokens = estimate_for_json(json);
    assert!(json_tokens > text_tokens, "json tokens should be higher due to repetitive structure");
}

#[test]
fn test_json_estimation_empty() {
    assert_eq!(estimate_for_json(""), 0);
}

#[test]
fn test_estimation_monotonicity() {
    // Longer input should always estimate more (or equal)
    assert!(estimate_tokens_for_text("a") <= estimate_tokens_for_text("ab"));
    assert!(estimate_tokens_for_text("ab") <= estimate_tokens_for_text("abc"));
    assert!(estimate_tokens_for_text("abc") <= estimate_tokens_for_text("abcd"));
}

// ── Model Limits Tests ───────────────────────────────────────

#[test]
fn test_usable_input_with_input_limit() {
    let limits = ModelLimits {
        context: 200_000,
        input: Some(200_000),
        output: 8_192,
    };
    assert_eq!(limits.usable_input(20_000), 180_000);
}

#[test]
fn test_usable_input_without_input_limit() {
    let limits = ModelLimits {
        context: 64_000,
        input: None,
        output: 8_192,
    };
    // Falls back to context when input is None
    assert_eq!(limits.usable_input(10_000), 54_000);
}

#[test]
fn test_usable_input_saturating() {
    let limits = ModelLimits {
        context: 10_000,
        input: Some(10_000),
        output: 2_048,
    };
    assert_eq!(limits.usable_input(20_000), 0);
}

#[test]
fn test_get_model_limits_known() {
    let limits = get_model_limits("claude-3-7-sonnet-20250219");
    assert!(limits.is_some());
    let limits = limits.unwrap();
    assert_eq!(limits.context, 200_000);
    assert_eq!(limits.output, 128_000);
}

#[test]
fn test_get_model_limits_case_insensitive() {
    assert!(get_model_limits("CLAUDE-3-7-SONNET-20250219").is_some());
    assert!(get_model_limits("Gpt-4o").is_some());
}

#[test]
fn test_get_model_limits_unknown() {
    assert!(get_model_limits("unknown-model").is_none());
}

#[test]
fn test_default_limits() {
    let d = get_default_limits();
    assert_eq!(d.context, 200_000);
    assert_eq!(d.input, Some(200_000));
    assert_eq!(d.output, 8_192);
}

#[test]
fn test_known_models_have_reasonable_limits() {
    for name in ["claude-3-7-sonnet-20250219", "gpt-4o", "gpt-4o-mini", "deepseek-chat"] {
        let limits = get_model_limits(name).unwrap_or_else(|| panic!("missing limits for {name}"));
        assert!(limits.context >= 50_000, "{name} context too small");
        assert!(limits.output >= 2_000, "{name} output too small");
    }
}

// ── Overflow Detection Tests ─────────────────────────────────

#[test]
fn test_overflow_low_usage() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 50_000, 10_000);
    assert!(!check.is_overflow());
}

#[test]
fn test_overflow_high_usage() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 190_000, 10_000);
    assert!(check.is_overflow());
}

#[test]
fn test_overflow_unknown_model() {
    let check = ContextOverflowCheck::new("unknown-model", 50_000, 10_000);
    assert!(!check.is_overflow());
}

#[test]
fn test_overflow_with_cache_tokens() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 50_000, 10_000)
        .with_cache(100_000, 0);
    assert_eq!(check.total_tokens, 160_000);
    assert_eq!(check.cache_read_tokens, 100_000);
    assert_eq!(check.cache_write_tokens, 0);
}

#[test]
fn test_overflow_new_total_is_input_plus_output() {
    let check = ContextOverflowCheck::new("test-model", 100_000, 50_000);
    assert_eq!(check.input_tokens, 100_000);
    assert_eq!(check.output_tokens, 50_000);
    assert_eq!(check.total_tokens, 150_000);
    assert_eq!(check.cache_read_tokens, 0);
    assert_eq!(check.cache_write_tokens, 0);
}

#[test]
fn test_overflow_boundary_exact_at_usable_limit() {
    // 200k - 20k buffer (min of reserved vs model name token) = ~180k usable
    // 180k input should be overflow
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 180_000, 0);
    assert!(check.is_overflow());
}

#[test]
fn test_overflow_zero_usage() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 0, 0);
    assert!(!check.is_overflow());
    assert_eq!(check.total_tokens, 0);
    assert_eq!(check.usage_ratio(), 0.0);
    assert_eq!(check.recommended_free(), COMPACTION_BUFFER);
}

#[test]
fn test_overflow_usage_ratio_under_half() {
    // 90k out of ~180k usable → ~0.5
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 90_000, 0);
    let ratio = check.usage_ratio();
    assert!((ratio - 0.5).abs() < 0.05);
}

#[test]
fn test_overflow_recommended_free_when_over() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 190_000, 10_000);
    assert!(check.recommended_free() > 0);
}

#[test]
fn test_overflow_recommended_free_when_under() {
    let check = ContextOverflowCheck::new("claude-3-7-sonnet-20250219", 10_000, 0);
    // Still recommends buffer as a compaction target
    assert!(check.recommended_free() > 0);
}

#[test]
fn test_is_context_overflow_helper_yes() {
    assert!(is_context_overflow("claude-3-7-sonnet-20250219", 200_000, 10_000));
}

#[test]
fn test_is_context_overflow_helper_no() {
    assert!(!is_context_overflow("claude-3-7-sonnet-20250219", 10_000, 10_000));
}
