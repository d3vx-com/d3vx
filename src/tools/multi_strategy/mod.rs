//! Multi-Strategy Execution Tool
//!
//! Generates multiple strategy-specific prompt variations for a task,
//! enabling the agent loop to try different implementation approaches
//! in parallel and pick the best result.

mod strategy;
#[cfg(test)]
mod tests;
mod tool;

pub use strategy::{clamp_max_agents, parse_strategies, Strategy};
pub use tool::MultiStrategyTool;
