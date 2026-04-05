//! Session store for conversation persistence
//!
//! Handles CRUD operations for conversation sessions used in the REPL,
//! one-shot mode, and pipeline phases.

mod store;
#[cfg(test)]
mod tests;
mod types;

pub use store::SessionStore;
pub use types::*;
