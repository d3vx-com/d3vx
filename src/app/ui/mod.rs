//! UI Rendering Module
//!
//! ## Architecture
//!
//! The TUI is rendered using ratatui with two main layers:
//!
//! 1. **Rendering layer** (`rendering/`) - All `impl App` render methods
//!    - Organized by component: messages, input, activity panel, sidebar, etc.
//!    - Methods are `impl App` blocks that render to a ratatui Frame
//!
//! 2. **Widget layer** (`ui/widgets/`) - Standalone reusable widgets
//!    - Independent of App state
//!    - Used by both the rendering layer and other parts of the codebase
//!
//! ## Rendering Layer Organization
//!
//! | Module | Purpose |
//! |--------|---------|
//! | `main_render` | Main render entry point, toast notifications |
//! | `welcome` | Welcome banner |
//! | `messages` | Chat message rendering |
//! | `input` | Input area and focus mode chips |
//! | `activity_panel` | Right panel with agents/tools |
//! | `activity_tabs` | Tab switching in activity panel |
//! | `activity_tools` | Tool execution display helpers |
//! | `agent_detail` | Inline agent inspector |
//! | `diff_preview` | Diff display in activity panel |
//! | `batch_detail` | Batch inspector for parallel agents |
//! | `sidebar` | Board/list sidebar |
//! | `command_palette` | Command palette overlay |
//! | `mention_picker` | File mention suggestions |
//! | `task_list` | Task list view |

// Helper utilities shared by rendering
pub mod helpers;

// Rendering layer - all impl App render methods
pub mod rendering;

// Re-export helpers for convenience
pub use helpers::*;
