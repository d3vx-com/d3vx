//! UI Module - Components and styling for the TUI

pub mod icons;
pub mod runner;
pub mod symbols;
pub mod theme;
pub mod widgets;

pub use runner::{is_ipc_mode, run_tui, TuiOptions};
pub use symbols::*;
pub use theme::{get_tool_color, Theme, ThemeMode};
