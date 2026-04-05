//! Message List Widget
//!
//! Displays conversation messages with proper formatting.
//!
//! Features:
//! - Compact tool call display with expandable details
//! - File change summaries
//! - Shell message support

use crate::ipc::Message;
use crate::ui::theme::Theme;

/// Configuration for truncation limits
pub struct TruncateConfig {
    pub command: usize,
    pub path: usize,
    pub query: usize,
    pub url: usize,
    pub diff_lines: usize,
    pub tool_output_lines: usize,
}

impl Default for TruncateConfig {
    fn default() -> Self {
        Self {
            command: 60,
            path: 50,
            query: 40,
            url: 50,
            diff_lines: 4,
            tool_output_lines: 20,
        }
    }
}

/// Spacing configuration
pub struct SpacingConfig {
    pub md: u16,
    pub sm: u16,
}

impl Default for SpacingConfig {
    fn default() -> Self {
        Self { md: 1, sm: 0 }
    }
}

/// Indent levels
pub struct IndentConfig {
    pub level1: u16,
    pub level2: u16,
}

impl Default for IndentConfig {
    fn default() -> Self {
        Self {
            level1: 2,
            level2: 4,
        }
    }
}

/// Message list widget
pub struct MessageList<'a> {
    pub(crate) messages: &'a [Message],
    pub(crate) verbose: bool,
    pub(crate) max_visible: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) theme: Theme,
    pub(crate) truncate: TruncateConfig,
}
