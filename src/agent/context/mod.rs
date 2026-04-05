//! Context Management Module
//!
//! Manages system prompt construction, caching, and auto-compaction.

pub mod compaction;
pub mod prompt_cache;

pub use compaction::{
    build_compaction_prompt, compact_conversation, needs_compaction, CompactionConfig,
    CompactionResult,
};
pub use prompt_cache::{CacheScope, PromptBlock, PromptCache};
