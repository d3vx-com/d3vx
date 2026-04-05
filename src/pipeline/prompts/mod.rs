//! Pipeline Phase Prompts
//!
//! Defines system prompts and instruction templates for each pipeline phase.
//! These prompts are used to guide the agent during phase execution.

#[cfg(test)]
mod tests;

pub mod instructions;
pub mod system_prompts;

// Re-export all public types for backward compatibility
pub use instructions::build_phase_instruction;
pub use system_prompts::get_system_prompt;
