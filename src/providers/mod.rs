//! LLM Provider Abstraction
//!
//! This module provides a unified interface for interacting with different LLM providers.
//! Each provider (Anthropic, OpenAI, etc.) implements the [`Provider`] trait.
//!
//! # Architecture
//!
//! ```text
//! providers/
//!   mod.rs        - Module exports
//!   types.rs      - Core types (messages, stream events, model info)
//!   traits.rs     - Provider trait definition
//!   anthropic/    - Anthropic (Claude) provider
//!   openai_compatible/ - OpenAI-compatible providers
//! ```

pub mod anthropic;
pub mod models;
pub mod openai_compatible;
pub mod pricing_cache;
pub mod registry;
pub mod traits;
pub mod types;

#[cfg(test)]
mod pricing_tests;

#[cfg(test)]
mod tests;

pub use models::ModelRegistry;
pub use registry::{ProviderInfo, ProviderRegistry, SUPPORTED_PROVIDERS};
pub use traits::{CostEstimate, Provider, ProviderError, StreamResult};
pub use types::{
    ComplexityTier, ContentBlock, ImageSource, Message, MessageContent, MessagesRequest, ModelInfo,
    ProviderOptions, ReasoningEffort, Role, StopReason, StreamEvent, ThinkingConfig, TokenUsage,
    ToolDefinition, ToolSchema,
};
