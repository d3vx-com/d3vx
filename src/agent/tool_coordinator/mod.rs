//! Tool Coordinator
//!
//! Manages tool registration and execution for the agent loop.
//! Provides a unified interface for registering, discovering, and
//! executing tools asynchronously.

mod coordinator;
#[cfg(test)]
mod tests;
mod types;

// Re-export all public types
pub use coordinator::{ToolCoordinator, ToolCoordinatorBuilder};
pub use types::{
    CoordinatorToolDefinition, SubAgentToolHandler, ToolCoordinatorError, ToolExecutionResult,
    ToolHandler,
};
