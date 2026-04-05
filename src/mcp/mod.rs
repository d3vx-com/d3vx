//! Model Context Protocol (MCP) Integration
//!
//! Provides support for the Model Context Protocol (MCP) to d3vx, allowing
//! for dynamic tool discovery and execution via standardized servers.

pub mod client;
pub mod manager;
pub mod protocol;

#[cfg(test)]
mod tests;

pub use client::McpClient;
pub use manager::McpManager;
pub use protocol::{CallToolResult, McpToolDefinition};
