//! Model Limits Configuration
//!
//! Per-model context window and output limits.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct ModelLimits {
    pub context: u64,
    pub input: Option<u64>,
    pub output: u64,
}

impl ModelLimits {
    pub fn usable_input(&self, reserved: u64) -> u64 {
        self.input.unwrap_or(self.context).saturating_sub(reserved)
    }
}

pub static MODEL_LIMITS: once_cell::sync::Lazy<HashMap<&'static str, ModelLimits>> =
    once_cell::sync::Lazy::new(|| {
        let mut m = HashMap::new();

        // Anthropic models
        m.insert(
            "claude-3-7-sonnet-20250219",
            ModelLimits {
                context: 200_000,
                input: Some(200_000),
                output: 128_000,
            },
        );
        m.insert(
            "claude-3-5-sonnet-20241022",
            ModelLimits {
                context: 200_000,
                input: Some(200_000),
                output: 8_192,
            },
        );
        m.insert(
            "claude-3-5-haiku-20241022",
            ModelLimits {
                context: 200_000,
                input: Some(200_000),
                output: 8_192,
            },
        );
        m.insert(
            "claude-3-opus-20240229",
            ModelLimits {
                context: 200_000,
                input: Some(200_000),
                output: 4_096,
            },
        );

        // OpenAI models
        m.insert(
            "gpt-4o",
            ModelLimits {
                context: 128_000,
                input: Some(128_000),
                output: 16_384,
            },
        );
        m.insert(
            "gpt-4o-mini",
            ModelLimits {
                context: 128_000,
                input: Some(128_000),
                output: 16_384,
            },
        );
        m.insert(
            "gpt-4-turbo",
            ModelLimits {
                context: 128_000,
                input: Some(128_000),
                output: 4_096,
            },
        );

        // DeepSeek
        m.insert(
            "deepseek-chat",
            ModelLimits {
                context: 64_000,
                input: None,
                output: 8_192,
            },
        );
        m.insert(
            "deepseek-coder",
            ModelLimits {
                context: 64_000,
                input: None,
                output: 8_192,
            },
        );

        m
    });

#[inline]
pub fn get_model_limits(model: &str) -> Option<&'static ModelLimits> {
    MODEL_LIMITS.get(&*model.to_lowercase())
}

#[inline]
pub fn get_default_limits() -> ModelLimits {
    ModelLimits {
        context: 200_000,
        input: Some(200_000),
        output: 8_192,
    }
}
