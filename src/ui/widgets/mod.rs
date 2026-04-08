//! UI Widgets - Reusable UI components

pub mod agent_view;
pub mod board;
pub mod diff_view;
pub mod docs_inspector;
pub mod help_modal;
pub mod inline_agents;
pub mod input;
pub mod markdown;
pub mod message_list;
pub mod model_picker;
pub mod session_picker;
pub mod shimmer;
pub mod thinking_indicator;
pub mod tool_display;
pub mod trust_panel;
pub mod undo_picker;

pub use agent_view::AgentView;
pub use diff_view::{DiffLine, DiffLineType, DiffView};
pub use docs_inspector::DocsInspector;
pub use help_modal::HelpModal;
pub use inline_agents::{InlineAgentCard, InlineAgentList};
pub use input::{InputState, InputWidget};
pub use markdown::{MarkdownConfig, MarkdownText};
pub use message_list::{IndentConfig, MessageList, SpacingConfig, TruncateConfig};
pub use session_picker::SessionPicker;
pub use shimmer::Shimmer;
pub use thinking_indicator::ThinkingIndicator;
pub use tool_display::{
    format_json_for_display, render_tool_summary, ToolDisplay, ToolDisplayConfig,
};
pub use trust_panel::TrustPanel;
pub use undo_picker::{UndoItem, UndoPicker};
