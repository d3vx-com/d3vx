//! Unified Plugin Architecture
//!
//! Consolidates all plugin systems into a single, coherent module:
//! - Extension plugins: Tool, Agent, Hook, Provider, UI
//! - Adapter plugins: Runtime, Workspace, SCM, Tracker, Notifier, Terminal, AgentBackend
//!
//! Architecture:
//! ```text
//! plugin/          -> Unified entry point
//!   ├── core.rs    -> Base Plugin trait + PluginContext
//!   ├── slots.rs   -> PluginSlot enum + all adapter traits
//!   ├── registry.rs -> Unified registry with typed accessors
//! ```

pub mod core;
pub mod registry;
pub mod slots;

pub use core::{Plugin, PluginContext, PluginError};
pub use registry::PluginRegistry;
pub use slots::{
    AdapterPlugin, AgentBackendAdapter, AgentHandle, CheckResult, IssueInfo, NotifierAdapter,
    PluginDescriptor, PluginSlot, PrInfo, PrStatus, ReviewInfo, RuntimeAdapter, ScmAdapter,
    TerminalAdapter, TerminalHandle, TrackerAdapter, WorkspaceAdapter,
};
