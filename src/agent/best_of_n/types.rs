//! Best-of-N types, configuration, and error definitions

use serde::{Deserialize, Serialize};

use crate::providers::TokenUsage;

/// Configuration for best-of-N execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestOfNConfig {
    /// Number of variants to generate
    pub n: usize,
    /// Selector prompt template
    pub selector_prompt: String,
    /// Model for variants (defaults to same as main)
    pub variant_model: Option<String>,
    /// Model for selector agent
    pub selector_model: Option<String>,
    /// Whether to include reasoning in variants
    pub strip_reasoning: bool,
}

impl Default for BestOfNConfig {
    fn default() -> Self {
        Self {
            n: 3,
            selector_prompt: DEFAULT_SELECTOR_PROMPT.to_string(),
            variant_model: None,
            selector_model: None,
            strip_reasoning: true,
        }
    }
}

const DEFAULT_SELECTOR_PROMPT: &str = r#"You are a code quality evaluator. Given multiple implementations of the same task, select the BEST one based on:
1. Correctness - Does it solve the problem?
2. Code quality - Is it well-structured and readable?
3. Efficiency - Is it performant?
4. Best practices - Does it follow language conventions?

Respond with ONLY the letter (A, B, C, etc.) of the best implementation."#;

/// Result of best-of-N execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestOfNResult {
    pub best_index: usize,
    pub best_content: String,
    pub alternatives: Vec<VariantResult>,
    pub selector_reasoning: Option<String>,
    pub total_tokens: TokenUsage,
}

/// A single variant result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResult {
    pub index: usize,
    pub content: String,
    pub tokens: TokenUsage,
    pub error: Option<String>,
}

/// Best-of-N errors
#[derive(Debug, thiserror::Error)]
pub enum BestOfNError {
    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("All variants failed")]
    AllVariantsFailed,

    #[error("No variants provided")]
    NoVariants,

    #[error("Selection failed: {0}")]
    SelectionFailed(String),
}
