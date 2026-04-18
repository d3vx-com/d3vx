//! Phase Handlers
//!
//! Defines the handler trait and implementations for each pipeline phase.
//! Each handler is responsible for executing the logic for its phase.

pub mod docs;
pub mod draft;
pub mod factory;
pub mod ideation;
pub mod impl_spec;
pub mod implement;
pub mod plan;
pub mod research;
pub mod review;
pub mod types;

pub use impl_spec::{ImplementationSpec, Subtask, SubtaskStatus};

#[cfg(test)]
mod tests;

// Re-export all public types
pub use docs::DocsHandler;
pub use draft::DraftHandler;
pub use factory::{create_handler, default_handlers};
pub use ideation::IdeationHandler;
pub use implement::ImplementHandler;
pub use plan::PlanHandler;
pub use research::ResearchHandler;
pub use review::ReviewHandler;
pub use types::{check_agent_safety, PhaseError, PhaseHandler, PhaseResult};
