//! Structured I/O for SDK Mode
//!
//! NDJSON stdin/stdout for programmatic d3vx usage.
//! Inspired by Claude Code's cli/structuredIO.ts.

mod events;
mod ndjson;
mod sdk;

pub use events::{ControlRequest, ControlResponse, SdkEvent, SdkResponse};
pub use ndjson::{NdJsonReader, NdJsonWriter};
pub use sdk::{SdkMode, SdkOptions};
