//! Best-of-N Execution Pattern
//!
//! Provides parallel generation with selector agent to pick the best result.

mod executor;
mod helpers;
mod selection;
mod types;

pub use executor::BestOfNExecutor;
pub use helpers::{strip_thinking_tags, truncate_preview};
pub use types::{BestOfNConfig, BestOfNError, BestOfNResult, VariantResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_preview() {
        let content = "a".repeat(100);
        let truncated = truncate_preview(&content, 50);
        assert!(truncated.contains("[truncated]"));
        assert!(truncated.len() < 100);
    }
}
