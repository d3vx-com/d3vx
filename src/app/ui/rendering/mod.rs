//! UI Rendering Module
//!
//! All rendering is implemented as `impl App` methods. This module contains
//! the main render loop and all panel/component renderers.
//!
//! ## Structure
//!
//! - `main_render` - Main render entry point and toast notifications
//! - `welcome` - Welcome banner rendering
//! - `messages` - Chat message rendering
//! - `activity_panel` - Activity panel (right side) with agents, tools, tabs
//! - `activity_tabs` - Activity panel tabs and main session detail
//! - `activity_tools` - Activity panel helpers (git changes, icon utilities)
//! - `agent_detail` - Agent detail inspector
//! - `diff_preview` - Compact diff preview in the activity panel
//! - `batch_detail` - Batch inspector for parallel agent batches
//! - `input` - Input area and status bar rendering
//! - `sidebar` - Sidebar rendering (board/list modes)
//! - `command_palette` - Command palette overlay
//! - `mention_picker` - File mention picker and permission request overlay
//! - `task_list` - Task list rendering

mod activity_panel;
mod activity_tabs;
mod activity_tools;
mod agent_detail;
mod agent_strip;
#[cfg(test)]
mod agent_strip_tests;
mod batch_detail;
mod command_palette;
mod diff_preview;
mod drawer;
mod input;
mod main_render;
mod mention_picker;
mod messages;
mod sidebar;
mod task_list;
mod welcome;
