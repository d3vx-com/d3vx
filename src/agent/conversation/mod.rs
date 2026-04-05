//! Conversation Management
//!
//! Manages message history for agent conversations. Supports both user
//! and assistant messages with structured content blocks.

mod history;
#[cfg(test)]
mod tests;

pub use history::Conversation;
