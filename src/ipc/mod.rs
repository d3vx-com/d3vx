//! IPC (Inter-Process Communication) Module
//!
//! Handles communication between the Rust TUI and the Node.js agent
//! via JSON-RPC over stdio.

pub mod client;
pub mod protocol;
pub mod sdk;
pub mod transport;
pub mod types;

pub use client::{parse_event, IpcClient, IpcEvent, IpcHandle};
pub use protocol::{Event, Method};
pub use sdk::{SdkEvent, SdkMode, SdkOptions, SdkResponse};
#[cfg(feature = "websocket")]
pub use transport::WebSocketTransport;
pub use transport::{HttpTransport, SseTransport, Transport, TransportError, TransportEvent};
pub use types::*;
