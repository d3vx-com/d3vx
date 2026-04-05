//! System Prompt Caching
//!
//! Splits the system prompt into static (cacheable) and dynamic (fresh) portions.
//! Static content (tool definitions, CLAUDE.md, project context) is tagged with
//! cache scope so providers can cache it and avoid re-charging tokens.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime};

/// Cache scope for a prompt block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheScope {
    /// Cache globally across all users/sessions (tool definitions, base prompts).
    Global,
    /// Cache at organization level (shared CLAUDE.md, project config).
    Organization,
    /// Cache at session level (conversation-specific context).
    Session,
    /// No caching -- this content changes every request.
    None,
}

/// A block of system prompt content with caching metadata.
#[derive(Debug, Clone)]
pub struct PromptBlock {
    /// The text content of this prompt block.
    pub text: String,
    /// Cache scope for this block.
    pub cache_scope: CacheScope,
    /// Label for debugging (e.g., "tool_definitions", "claude_md", "dynamic_context").
    pub label: String,
    /// When this block was last modified.
    pub last_modified: SystemTime,
}

impl PromptBlock {
    /// Create a new prompt block with the current time as `last_modified`.
    pub fn new(text: impl Into<String>, cache_scope: CacheScope, label: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cache_scope,
            label: label.into(),
            last_modified: SystemTime::now(),
        }
    }
}

/// Boundary marker between static and dynamic content.
pub const DYNAMIC_BOUNDARY: &str = "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__";

/// Default time-to-live for static content before forced rebuild.
const DEFAULT_STATIC_TTL: Duration = Duration::from_secs(300);

/// Manages system prompt assembly with caching support.
///
/// Static blocks (tool definitions, CLAUDE.md) are tagged with cache scopes so
/// providers that support prompt caching (e.g., Anthropic) can avoid re-charging
/// tokens. Dynamic blocks (date, git status) are regenerated every request.
#[derive(Debug)]
pub struct PromptCache {
    /// Static blocks that rarely change (tool defs, CLAUDE.md).
    static_blocks: Vec<PromptBlock>,
    /// Dynamic blocks that change every request (date, git status).
    dynamic_blocks: Vec<PromptBlock>,
    /// Hash of static content for cache invalidation.
    static_hash: u64,
    /// Last time static blocks were rebuilt.
    static_rebuilt_at: Option<SystemTime>,
    /// TTL for static content before forced rebuild.
    static_ttl: Duration,
}

impl Default for PromptCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptCache {
    /// Create a new, empty prompt cache with default TTL (5 minutes).
    pub fn new() -> Self {
        Self {
            static_blocks: Vec::new(),
            dynamic_blocks: Vec::new(),
            static_hash: 0,
            static_rebuilt_at: None,
            static_ttl: DEFAULT_STATIC_TTL,
        }
    }

    /// Create a prompt cache with a custom TTL for static content.
    pub fn with_ttl(static_ttl: Duration) -> Self {
        Self {
            static_blocks: Vec::new(),
            dynamic_blocks: Vec::new(),
            static_hash: 0,
            static_rebuilt_at: None,
            static_ttl,
        }
    }

    /// Replace all static blocks and recompute the content hash.
    pub fn set_static_blocks(&mut self, blocks: Vec<PromptBlock>) {
        self.static_hash = compute_hash(&blocks);
        self.static_blocks = blocks;
        self.static_rebuilt_at = Some(SystemTime::now());
    }

    /// Add a dynamic block for the current request.
    pub fn add_dynamic_block(&mut self, block: PromptBlock) {
        self.dynamic_blocks.push(block);
    }

    /// Clear all dynamic blocks. Call at the start of each new request.
    pub fn clear_dynamic(&mut self) {
        self.dynamic_blocks.clear();
    }

    /// Assemble the full system prompt with cache markers.
    ///
    /// Returns blocks in order: static (preserving their cache scopes),
    /// a boundary marker, then dynamic blocks all set to `CacheScope::None`.
    pub fn assemble(&self) -> Vec<PromptBlock> {
        let mut result =
            Vec::with_capacity(self.static_blocks.len() + self.dynamic_blocks.len() + 1);

        // Static blocks keep their original cache scopes.
        result.extend(self.static_blocks.iter().cloned());

        // Boundary marker so consumers can split static from dynamic.
        result.push(PromptBlock::new(
            DYNAMIC_BOUNDARY,
            CacheScope::None,
            "boundary",
        ));

        // Dynamic blocks are always no-cache.
        for block in &self.dynamic_blocks {
            let mut dynamic_block = block.clone();
            dynamic_block.cache_scope = CacheScope::None;
            result.push(dynamic_block);
        }

        result
    }

    /// Check if static content needs rebuilding because it has exceeded its TTL.
    pub fn needs_rebuild(&self) -> bool {
        match self.static_rebuilt_at {
            Some(rebuilt_at) => {
                rebuilt_at.elapsed().unwrap_or(DEFAULT_STATIC_TTL) >= self.static_ttl
            }
            None => true,
        }
    }

    /// Get total estimated token count for all blocks (static + dynamic).
    ///
    /// Uses a simple heuristic of 4 characters per token.
    pub fn estimated_tokens(&self) -> u64 {
        let static_chars: usize = self.static_blocks.iter().map(|b| b.text.len()).sum();
        let dynamic_chars: usize = self.dynamic_blocks.iter().map(|b| b.text.len()).sum();
        chars_to_tokens(static_chars + dynamic_chars)
    }

    /// Get the number of cacheable tokens (static blocks only).
    pub fn cacheable_tokens(&self) -> u64 {
        let chars: usize = self.static_blocks.iter().map(|b| b.text.len()).sum();
        chars_to_tokens(chars)
    }

    /// Get the current static content hash for cache invalidation checks.
    pub fn static_hash(&self) -> u64 {
        self.static_hash
    }

    /// Get the number of static blocks.
    pub fn static_block_count(&self) -> usize {
        self.static_blocks.len()
    }

    /// Get the number of dynamic blocks.
    pub fn dynamic_block_count(&self) -> usize {
        self.dynamic_blocks.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a character count to an approximate token count.
fn chars_to_tokens(chars: usize) -> u64 {
    (chars as u64) / 4
}

/// Compute a deterministic hash over the text of all blocks.
fn compute_hash(blocks: &[PromptBlock]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for block in blocks {
        block.text.hash(&mut hasher);
    }
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn static_block(label: &str, text: impl Into<String>) -> PromptBlock {
        PromptBlock::new(text, CacheScope::Global, label)
    }

    fn dynamic_block(label: &str, text: impl Into<String>) -> PromptBlock {
        PromptBlock::new(text, CacheScope::None, label)
    }

    #[test]
    fn assemble_with_static_and_dynamic() {
        let mut cache = PromptCache::new();
        cache.set_static_blocks(vec![
            static_block("tools", "You have the following tools: Read, Write."),
            static_block("claude_md", "Project: d3vx-terminal."),
        ]);
        cache.add_dynamic_block(dynamic_block("date", "Today is 2026-03-29."));
        cache.add_dynamic_block(dynamic_block("git_status", "M src/main.rs"));

        let assembled = cache.assemble();

        // 2 static + 1 boundary + 2 dynamic = 5
        assert_eq!(assembled.len(), 5);

        // Static blocks keep their cache scope.
        assert_eq!(assembled[0].cache_scope, CacheScope::Global);
        assert_eq!(assembled[0].label, "tools");
        assert_eq!(assembled[1].cache_scope, CacheScope::Global);
        assert_eq!(assembled[1].label, "claude_md");

        // Boundary marker.
        assert_eq!(assembled[2].text, DYNAMIC_BOUNDARY);
        assert_eq!(assembled[2].cache_scope, CacheScope::None);

        // Dynamic blocks.
        assert_eq!(assembled[3].cache_scope, CacheScope::None);
        assert_eq!(assembled[3].label, "date");
        assert_eq!(assembled[4].cache_scope, CacheScope::None);
        assert_eq!(assembled[4].label, "git_status");
    }

    #[test]
    fn cache_scope_preservation() {
        let mut cache = PromptCache::new();
        cache.set_static_blocks(vec![
            PromptBlock::new("global stuff", CacheScope::Global, "g"),
            PromptBlock::new("org stuff", CacheScope::Organization, "o"),
            PromptBlock::new("session stuff", CacheScope::Session, "s"),
        ]);

        let assembled = cache.assemble();
        // 3 static + 1 boundary = 4
        assert_eq!(assembled.len(), 4);
        assert_eq!(assembled[0].cache_scope, CacheScope::Global);
        assert_eq!(assembled[1].cache_scope, CacheScope::Organization);
        assert_eq!(assembled[2].cache_scope, CacheScope::Session);
    }

    #[test]
    fn token_estimation() {
        let mut cache = PromptCache::new();
        // 40 chars -> 10 tokens
        cache.set_static_blocks(vec![static_block("a", "A".repeat(40))]);
        // 20 chars -> 5 tokens
        cache.add_dynamic_block(dynamic_block("b", "B".repeat(20)));

        // Total: 60 chars / 4 = 15 tokens
        assert_eq!(cache.estimated_tokens(), 15);
        // Cacheable: 40 chars / 4 = 10 tokens
        assert_eq!(cache.cacheable_tokens(), 10);
    }

    #[test]
    fn needs_rebuild_when_empty() {
        let cache = PromptCache::new();
        // Never built -> always needs rebuild.
        assert!(cache.needs_rebuild());
    }

    #[test]
    fn needs_rebuild_within_ttl() {
        let mut cache = PromptCache::new();
        cache.set_static_blocks(vec![static_block("x", "content")]);

        // Just built -> should not need rebuild.
        assert!(!cache.needs_rebuild());
    }

    #[test]
    fn needs_rebuild_after_ttl_expires() {
        let mut cache = PromptCache::with_ttl(Duration::from_millis(1));
        cache.set_static_blocks(vec![static_block("x", "content")]);

        // Wait for TTL to expire.
        std::thread::sleep(Duration::from_millis(5));

        assert!(cache.needs_rebuild());
    }

    #[test]
    fn clear_dynamic_removes_all_dynamic_blocks() {
        let mut cache = PromptCache::new();
        cache.set_static_blocks(vec![static_block("s", "static")]);
        cache.add_dynamic_block(dynamic_block("d1", "dynamic 1"));
        cache.add_dynamic_block(dynamic_block("d2", "dynamic 2"));

        assert_eq!(cache.dynamic_block_count(), 2);

        cache.clear_dynamic();

        assert_eq!(cache.dynamic_block_count(), 0);

        let assembled = cache.assemble();
        // 1 static + 1 boundary = 2 (no dynamic blocks left)
        assert_eq!(assembled.len(), 2);
    }

    #[test]
    fn static_hash_changes_on_update() {
        let mut cache = PromptCache::new();
        cache.set_static_blocks(vec![static_block("a", "alpha")]);
        let hash_v1 = cache.static_hash();

        cache.set_static_blocks(vec![static_block("a", "beta")]);
        let hash_v2 = cache.static_hash();

        assert_ne!(hash_v1, hash_v2);
    }

    #[test]
    fn block_count_accessors() {
        let mut cache = PromptCache::new();
        assert_eq!(cache.static_block_count(), 0);
        assert_eq!(cache.dynamic_block_count(), 0);

        cache.set_static_blocks(vec![static_block("s1", "x"), static_block("s2", "y")]);
        cache.add_dynamic_block(dynamic_block("d1", "z"));

        assert_eq!(cache.static_block_count(), 2);
        assert_eq!(cache.dynamic_block_count(), 1);
    }

    #[test]
    fn dynamic_blocks_always_none_scope_in_assembly() {
        let mut cache = PromptCache::new();
        // Force a non-None scope on a dynamic block (shouldn't happen in practice).
        cache.add_dynamic_block(PromptBlock::new(
            "leaked scope",
            CacheScope::Global,
            "should_be_none",
        ));

        let assembled = cache.assemble();
        // The dynamic block is after the boundary (index 1).
        let dynamic = &assembled[1]; // index 0 is boundary (no static blocks)
        assert_eq!(dynamic.cache_scope, CacheScope::None);
    }
}
