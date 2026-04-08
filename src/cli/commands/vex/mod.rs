//! Vex Mode Module
//!
//! Handles `d3vx --vex "task description"` — creates a background
//! autonomous task that runs in an isolated tmux session.

pub mod tools;
pub mod handler;

pub use handler::{run_task_detached, run_vex_mode};
