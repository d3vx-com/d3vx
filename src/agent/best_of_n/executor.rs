//! Best-of-N executor implementation

use std::sync::Arc;
use tracing::{debug, info};

use super::helpers::strip_thinking_tags;
use super::selection::select_best_with_prompt;
use super::types::*;
use crate::providers::{Provider, TokenUsage};

/// Best-of-N executor
pub struct BestOfNExecutor {
    pub(crate) provider: Arc<dyn Provider>,
    pub(crate) config: BestOfNConfig,
}

impl BestOfNExecutor {
    /// Create a new best-of-N executor
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            config: BestOfNConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(provider: Arc<dyn Provider>, config: BestOfNConfig) -> Self {
        Self { provider, config }
    }

    /// Execute best-of-N with parallel generation
    pub async fn execute(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<BestOfNResult, BestOfNError> {
        let n = self.config.n;

        info!("Starting best-of-{} execution", n);

        // Generate N variants in parallel
        let variant_futures: Vec<_> = (0..n)
            .map(|i| self.generate_variant(i, prompt, system_prompt))
            .collect();

        let results = futures::future::join_all(variant_futures).await;

        // Collect successful results
        let mut alternatives: Vec<VariantResult> = Vec::new();
        for result in results {
            if let Ok(variant) = result {
                alternatives.push(variant);
            }
        }

        if alternatives.is_empty() {
            return Err(BestOfNError::AllVariantsFailed);
        }

        // If only one variant succeeded, return it
        if alternatives.len() == 1 {
            let best = alternatives.remove(0);
            return Ok(BestOfNResult {
                best_index: best.index,
                best_content: best.content,
                alternatives,
                selector_reasoning: None,
                total_tokens: best.tokens,
            });
        }

        // Select the best variant
        let (best_index, reasoning) = self.select_best(&alternatives, prompt).await?;

        // Calculate total tokens
        let total_tokens = alternatives
            .iter()
            .fold(TokenUsage::default(), |mut acc, v| {
                acc.input_tokens += v.tokens.input_tokens;
                acc.output_tokens += v.tokens.output_tokens;
                acc
            });

        let best = &alternatives[best_index];

        Ok(BestOfNResult {
            best_index: best.index,
            best_content: best.content.clone(),
            alternatives,
            selector_reasoning: reasoning,
            total_tokens,
        })
    }

    /// Evaluate existing candidate outputs and select the best one.
    pub async fn select_existing_variants(
        &self,
        original_prompt: &str,
        variants: &[VariantResult],
        selector_prompt_override: Option<&str>,
    ) -> Result<(usize, Option<String>), BestOfNError> {
        select_best_with_prompt(
            &self.provider,
            &self.config,
            variants,
            original_prompt,
            selector_prompt_override,
        )
        .await
    }

    /// Select the best variant using a selector agent
    async fn select_best(
        &self,
        variants: &[VariantResult],
        original_prompt: &str,
    ) -> Result<(usize, Option<String>), BestOfNError> {
        select_best_with_prompt(
            &self.provider,
            &self.config,
            variants,
            original_prompt,
            None,
        )
        .await
    }

    /// Generate a single variant
    async fn generate_variant(
        &self,
        index: usize,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<VariantResult, BestOfNError> {
        let variant_prompt = format!(
            "{}\n\n[Variant {} of {} - Please provide your best implementation.]",
            prompt,
            index + 1,
            self.config.n
        );

        let start = std::time::Instant::now();

        let request = crate::providers::MessagesRequest {
            model: self
                .config
                .variant_model
                .clone()
                .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
            messages: vec![crate::providers::Message {
                role: crate::providers::Role::User,
                content: crate::providers::MessageContent::Text(variant_prompt),
            }],
            system_prompt: system_prompt.map(|s| s.to_string()),
            tools: vec![],
            max_tokens: None,
            temperature: Some(0.7),
            thinking: None,
            prompt_caching: true,
        };

        let mut stream = self
            .provider
            .send(request)
            .await
            .map_err(|e| BestOfNError::ProviderError(e.to_string()))?;

        let mut content = String::new();
        let mut usage = crate::providers::TokenUsage::default();

        use futures::StreamExt;
        while let Some(event) = stream.next().await {
            match event {
                Ok(crate::providers::StreamEvent::TextDelta { text }) => {
                    content.push_str(&text);
                }
                Ok(crate::providers::StreamEvent::MessageEnd { usage: u, .. }) => {
                    usage = u;
                }
                Err(e) => {
                    return Err(BestOfNError::StreamError(e.to_string()));
                }
                _ => {}
            }
        }

        let elapsed = start.elapsed();
        debug!("Variant {} generated in {:?}", index, elapsed);

        let content = if self.config.strip_reasoning {
            strip_thinking_tags(&content)
        } else {
            content
        };

        Ok(VariantResult {
            index,
            content,
            tokens: usage,
            error: None,
        })
    }
}
