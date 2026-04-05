//! Provider and model routing configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a specific LLM provider
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ProviderConfig {
    /// Environment variable name for API key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Default model for this provider
    pub default_model: String,
    /// Base URL for API requests (for proxies or local models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Model to use for research phase (cheaper/faster)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub research_model: Option<String>,
    /// Cheap/fast model for low-stakes tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cheap_model: Option<String>,
    /// Request timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Maximum retries on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
}

/// Providers configuration including fallback chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ProvidersConfig {
    /// Fallback chain - ordered list of provider:model entries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_chain: Option<Vec<String>>,
    /// Provider-specific configurations (keyed by provider name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<HashMap<String, ProviderConfig>>,
}

/// Configuration for tier-based model routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ModelRouting {
    /// Enable automatic model routing based on phase
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cheap/fast model for lightweight tasks.
    #[serde(skip_serializing_if = "Option::is_none", alias = "trivial_model")]
    pub cheap_model: Option<String>,

    /// Standard model for most implementation work.
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "implementation_model",
        alias = "fallback_model"
    )]
    pub standard_model: Option<String>,

    /// Premium model for planning, synthesis, and high-risk review.
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "planning_model",
        alias = "review_model",
        alias = "research_model"
    )]
    pub premium_model: Option<String>,

    /// Enable complexity-based routing
    #[serde(default = "default_true")]
    pub complexity_routing: bool,
}

fn default_true() -> bool {
    true
}
