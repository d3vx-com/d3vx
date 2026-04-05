//! Message List Widget
//!
//! Displays conversation messages with proper formatting.
//!
//! Features:
//! - Compact tool call display with expandable details
//! - File change summaries
//! - Shell message support

pub mod rendering;
pub mod tool_rendering;
pub mod types;

#[cfg(test)]
mod tests;

pub use types::{IndentConfig, MessageList, SpacingConfig, TruncateConfig};
