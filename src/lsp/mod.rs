//! Language Server Protocol Integration
//!
//! Provides LSP client for diagnostics, completion, and navigation.

mod bridge;
mod client;
mod completion;
mod diagnostics;
mod goto_def;

pub use bridge::{LspBridge, LspBridgeConfig, LspDiagnostic, LspSeverity};
pub use client::LspClient;
pub use completion::CompletionProvider;
pub use diagnostics::DiagnosticManager;
pub use goto_def::GotoProvider;
