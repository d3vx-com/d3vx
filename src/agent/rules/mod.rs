//! Per-Project Agent Rules
//!
//! Loads project-specific rules from `.d3vx/rules.yaml` and/or
//! `docs/ARCHITECTURE.md` to inject into agent context.

mod loader;
mod types;

#[cfg(test)]
mod tests;

pub use types::ProjectRules;
