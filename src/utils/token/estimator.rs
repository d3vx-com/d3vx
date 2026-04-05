//! Token Estimation Utilities
//!
//! Character-based token estimation with model-specific adjustments.
//! OpenCode parity: 4 chars/token baseline, adjusted per content type.

const CHARS_PER_TOKEN: usize = 4;

/// Estimate tokens for a string using character-based approximation.
#[inline]
pub fn estimate_tokens_for_text(text: &str) -> usize {
    text.len().saturating_sub(text.len() % CHARS_PER_TOKEN) / CHARS_PER_TOKEN
}

/// Estimate tokens for a message content type.
#[inline]
pub fn estimate_for_content(content: &str) -> usize {
    estimate_tokens_for_text(content)
}

/// Adjust estimate for code (typically has longer tokens).
#[inline]
pub fn estimate_for_code(code: &str) -> usize {
    (code.len() as f64 / (CHARS_PER_TOKEN as f64 * 1.2)) as usize
}

/// Adjust estimate for JSON (has repetitive structure).
#[inline]
pub fn estimate_for_json(json: &str) -> usize {
    (json.len() as f64 / (CHARS_PER_TOKEN as f64 * 0.9)) as usize
}
