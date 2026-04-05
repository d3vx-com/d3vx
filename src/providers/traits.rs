//! Provider Trait Definition
//!
//! The [`Provider`] trait defines the interface that all LLM providers must implement.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use super::{MessagesRequest, ModelInfo, StreamEvent, TokenUsage};

pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

/// Universal LLM provider interface.
///
/// All providers implement this trait, providing a consistent API
/// regardless of the underlying LLM service.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a messages request and return a stream of events.
    ///
    /// # Arguments
    ///
    /// * `request` - The messages request containing model, messages, tools, etc.
    ///
    /// # Returns
    ///
    /// A stream of [`StreamEvent`]s that can be consumed asynchronously.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the request fails to initiate or
    /// if an error occurs during streaming.
    async fn send(&self, request: MessagesRequest) -> Result<StreamResult, ProviderError>;

    /// Provider identifier (e.g., "anthropic", "openai", "ollama").
    fn name(&self) -> &str;

    /// List available models for this provider.
    fn models(&self) -> Vec<ModelInfo>;

    /// Dynamically discover available models from the provider's API.
    ///
    /// Useful for providers with dynamic model sets like Ollama or OpenRouter.
    /// Default implementation returns the static list from `models()`.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self.models())
    }

    /// Discover models from Ollama instance.
    async fn discover_ollama(&self, _base_url: &str) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self.models())
    }

    /// Discover models from OpenRouter.
    async fn discover_openrouter(
        &self,
        _base_url: &str,
        _api_key: &str,
    ) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self.models())
    }

    /// Get metadata for a specific model.
    ///
    /// Returns `None` if the model is not recognized.
    fn model_info(&self, model_id: &str) -> Option<ModelInfo>;

    /// Approximate token count for text.
    ///
    /// Default implementation uses character approximation (~4 chars per token).
    fn count_tokens(&self, text: &str, _model: Option<&str>) -> usize {
        text.len() / 4
    }

    /// Check if the provider is configured and ready to use.
    fn is_available(&self) -> bool;

    /// Whether this provider supports prompt caching (cache_control breakpoints).
    fn supports_prompt_caching(&self) -> bool {
        false
    }

    /// Calculate cost estimate from token usage.
    fn estimate_cost(&self, model: &str, usage: &TokenUsage) -> Option<CostEstimate>;
}

/// Cost estimation for API usage.
#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
}

// ============================================================================
// Provider Error
// ============================================================================

/// Error type for provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Context window exceeded: {0}")]
    ContextExceeded(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Provider unavailable: {0}")]
    Unavailable(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Timeout")]
    Timeout,

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl ProviderError {
    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::RateLimited(_)
            | ProviderError::Unavailable(_)
            | ProviderError::Network(_)
            | ProviderError::Timeout => true,
            // Http errors from reqwest are retryable if they are connection/timeout/server related
            ProviderError::Http(e) => {
                e.is_connect()
                    || e.is_timeout()
                    || e.is_request() && e.status().map(|s| s.is_server_error()).unwrap_or(false)
            }
            // StreamError is retryable if it contains network-related messages
            ProviderError::StreamError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("network")
                    || msg_lower.contains("connection")
                    || msg_lower.contains("timeout")
                    || msg_lower.contains("reset")
                    || msg_lower.contains("broken pipe")
                    || msg_lower.contains("eof")
                    || msg_lower.contains("unexpected")
            }
            _ => false,
        }
    }

    /// Get the suggested retry delay in milliseconds.
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited(_) => Some(60_000), // 1 minute
            ProviderError::Unavailable(_) => Some(30_000), // 30 seconds
            ProviderError::Network(_) => Some(5_000),      // 5 seconds
            ProviderError::Timeout => Some(10_000),        // 10 seconds
            ProviderError::StreamError(_) => Some(5_000), // 5 seconds for network-related stream errors
            ProviderError::Http(e) => {
                if e.is_timeout() {
                    Some(10_000)
                } else {
                    Some(5_000)
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(ProviderError::RateLimited("".to_string()).is_retryable());
        assert!(ProviderError::Network("".to_string()).is_retryable());
        assert!(ProviderError::Timeout.is_retryable());
        assert!(ProviderError::Unavailable("".to_string()).is_retryable());

        // StreamError matching
        assert!(ProviderError::StreamError("network error".to_string()).is_retryable());
        assert!(ProviderError::StreamError("Connection reset by peer".to_string()).is_retryable());
        assert!(!ProviderError::StreamError("normal end".to_string()).is_retryable());

        // Auth failed should NOT be retryable
        assert!(!ProviderError::AuthFailed("".to_string()).is_retryable());
    }

    #[test]
    fn test_retry_delay() {
        assert_eq!(
            ProviderError::RateLimited("".to_string()).retry_delay_ms(),
            Some(60_000)
        );
        assert_eq!(
            ProviderError::Network("".to_string()).retry_delay_ms(),
            Some(5_000)
        );
        assert_eq!(ProviderError::Timeout.retry_delay_ms(), Some(10_000));
        assert_eq!(
            ProviderError::AuthFailed("".to_string()).retry_delay_ms(),
            None
        );
    }
}
