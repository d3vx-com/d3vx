//! CLI Module - Command-line argument parsing and routing
//!
//! This module provides the CLI interface for d3vx, using clap's derive macros
//! to define commands and arguments. It mirrors the TypeScript CLI commands.

pub mod args;
pub mod commands;

pub use args::{Cli, CliCommand};
pub use commands::execute;

#[cfg(test)]
mod tests;
