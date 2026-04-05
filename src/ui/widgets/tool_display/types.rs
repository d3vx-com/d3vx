//! Tool display types and configuration

use crate::ipc::ToolStatus;
use crate::ui::theme::Theme;

/// Configuration for tool display rendering
pub struct ToolDisplayConfig {
    /// Maximum lines to show for output
    pub max_output_lines: usize,
    /// Maximum characters for input preview
    pub max_input_preview: usize,
    /// Show timing information
    pub show_timing: bool,
    /// Verbose mode (show full output)
    pub verbose: bool,
    /// Maximum width for truncation
    pub max_width: usize,
}

impl Default for ToolDisplayConfig {
    fn default() -> Self {
        Self {
            max_output_lines: 10,
            max_input_preview: 60,
            show_timing: true,
            verbose: false,
            max_width: 80,
        }
    }
}

/// Widget for rendering tool use/result blocks
pub struct ToolDisplay<'a> {
    /// Tool name
    pub(crate) name: &'a str,
    /// Tool ID
    pub(crate) _id: &'a str,
    /// Tool input (JSON)
    pub(crate) input: &'a serde_json::Value,
    /// Tool status
    pub(crate) status: ToolStatus,
    /// Tool output (if completed)
    pub(crate) output: Option<&'a str>,
    /// Execution time in milliseconds
    pub(crate) elapsed_ms: Option<u64>,
    /// Theme
    pub(crate) theme: Theme,
    /// Configuration
    pub(crate) config: ToolDisplayConfig,
    /// Animation frame (for spinner)
    pub(crate) animation_frame: u64,
}

impl<'a> ToolDisplay<'a> {
    /// Create a new tool display
    pub fn new(
        name: &'a str,
        id: &'a str,
        input: &'a serde_json::Value,
        status: ToolStatus,
    ) -> Self {
        Self {
            name,
            _id: id,
            input,
            status,
            output: None,
            elapsed_ms: None,
            theme: Theme::dark(),
            config: ToolDisplayConfig::default(),
            animation_frame: 0,
        }
    }

    /// Set the output
    pub fn output(mut self, output: Option<&'a str>) -> Self {
        self.output = output;
        self
    }

    /// Set elapsed time
    pub fn elapsed(mut self, ms: u64) -> Self {
        self.elapsed_ms = Some(ms);
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Set config
    pub fn config(mut self, config: ToolDisplayConfig) -> Self {
        self.config = config;
        self
    }

    /// Set max width
    pub fn max_width(mut self, width: usize) -> Self {
        self.config.max_width = width;
        self
    }

    /// Set animation frame
    pub fn animation_frame(mut self, frame: u64) -> Self {
        self.animation_frame = frame;
        self
    }
}
