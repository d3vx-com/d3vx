//! Transport Abstraction Layer
//!
//! Provides pluggable transports for different communication patterns.
//! Inspired by Claude Code's transport layer (cli/transports/).

mod http;
mod sse;
mod traits;
#[cfg(feature = "websocket")]
mod websocket;

pub use http::HttpTransport;
pub use sse::SseTransport;
pub use traits::{Transport, TransportError, TransportEvent};
#[cfg(feature = "websocket")]
pub use websocket::WebSocketTransport;
