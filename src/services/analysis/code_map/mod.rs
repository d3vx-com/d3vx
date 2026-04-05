//! Code Map Module
//!
//! Indexes source files and ranks them by relevance to queries.

pub mod scoring;
#[cfg(test)]
pub mod tests;
pub mod types;

// Re-export all public items at the module level.
pub use scoring::{
    apply_call_bonus, build_code_map, compute_base_score, depth_penalty, rank_files_for_query,
    size_factor, tokenize,
};
pub use types::{CodeMap, FileEntry, ScoredFile};
