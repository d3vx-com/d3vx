//! d3vx TUI Library
//!
//! This crate provides the terminal UI components for d3vx.

pub mod agent;
pub mod app;
pub mod cli;
pub mod config;
pub mod event;
pub mod hooks;
pub mod ipc;
pub mod lsp;
pub mod mcp;
pub mod metrics;
pub mod notifications;
pub mod pipeline;
pub mod plugin;
pub mod providers;
pub mod recovery;
pub mod services;
pub mod skills;
pub mod store;
pub mod team;
pub mod tools;
pub mod types;
pub mod ui;
pub mod utils;

// Re-exports
pub use app::App;
pub use cli::{execute, Cli, CliCommand};
pub use cli::commands::AppError;
pub use config::{get_api_key, get_provider_config, load_config, D3vxConfig, LoadConfigOptions};
pub use pipeline::{
    create_handler, default_handlers, DocsHandler, ImplementHandler, Phase, PhaseContext,
    PhaseError, PhaseHandler, PhaseResult, PipelineConfig, PipelineEngine, PipelineRunResult,
    PlanHandler, Priority, QueueError, QueueStats, ResearchHandler, ReviewHandler, Task, TaskQueue,
    TaskStatus,
};
pub use providers::{ContentBlock, Message, Role, ToolDefinition};
pub use tools::{register_core_tools, Tool, ToolContext, ToolResult};
pub use ui::theme::{Theme, ThemeMode};
