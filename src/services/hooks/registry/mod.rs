//! Hook registry for managing and running pre-commit hooks
//!
//! The registry manages a collection of hooks and runs
//! them as part of the pre-commit process.

mod impl_registry;
mod types;

#[cfg(test)]
mod tests;

pub use impl_registry::HookRegistry;
pub use types::{HookRegistryConfig, HookRunInfo, HooksRunResult};
