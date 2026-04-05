//! Batch Spawn with Issue Context
//!
//! Launches multiple agent sessions from external issue tracker items,
//! generating branches, composing prompts, and managing concurrency.

pub mod issue_launcher;
pub mod prompt_composer;
pub mod types;

pub use issue_launcher::*;
pub use prompt_composer::*;
pub use types::*;
