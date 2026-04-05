//! Tool Display Widget
//!
//! Renders tool use and tool result blocks in the message list.
//! Shows execution status, timing, and collapsible output.

pub mod helpers;
pub mod rendering;
pub mod types;

#[cfg(test)]
mod tests;

pub use helpers::{format_json_for_display, render_tool_summary};
pub use types::{ToolDisplay, ToolDisplayConfig};
