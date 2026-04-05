//! Context Overflow Detection
//!
//! Determines when context compaction is needed.
//! OpenCode parity: reserves buffer for response headroom.

use super::model_limits::{get_default_limits, get_model_limits};

pub const COMPACTION_BUFFER: u64 = 20_000;

#[derive(Debug, Clone)]
pub struct ContextOverflowCheck {
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub model: String,
    pub reserved: u64,
}

impl ContextOverflowCheck {
    pub fn new(model: &str, input: u64, output: u64) -> Self {
        Self {
            total_tokens: input + output,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            model: model.to_string(),
            reserved: COMPACTION_BUFFER,
        }
    }

    pub fn with_cache(mut self, read: u64, write: u64) -> Self {
        self.cache_read_tokens = read;
        self.cache_write_tokens = write;
        self.total_tokens = self.input_tokens + self.output_tokens + read + write;
        self
    }

    /// Check if context exceeds usable limit.
    pub fn is_overflow(&self) -> bool {
        let limits = get_model_limits(&self.model)
            .copied()
            .unwrap_or_else(get_default_limits);
        let reserved = self
            .reserved
            .min(super::estimator::estimate_tokens_for_text(&self.model) as u64);
        let usable = limits.usable_input(reserved);
        self.total_tokens >= usable
    }

    /// Get percentage of context window used.
    pub fn usage_ratio(&self) -> f64 {
        let limits = get_model_limits(&self.model)
            .copied()
            .unwrap_or_else(get_default_limits);
        let usable = limits.usable_input(self.reserved);
        if usable == 0 {
            return 1.0;
        }
        self.total_tokens as f64 / usable as f64
    }

    /// Recommended tokens to free up.
    pub fn recommended_free(&self) -> u64 {
        let limits = get_model_limits(&self.model)
            .copied()
            .unwrap_or_else(get_default_limits);
        let usable = limits.usable_input(self.reserved);
        self.total_tokens.saturating_sub(usable) + COMPACTION_BUFFER
    }
}

#[inline]
pub fn is_context_overflow(model: &str, input: u64, output: u64) -> bool {
    ContextOverflowCheck::new(model, input, output).is_overflow()
}
