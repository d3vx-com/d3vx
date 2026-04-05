//! Handler factory functions

use super::docs::DocsHandler;
use super::draft::DraftHandler;
use super::ideation::IdeationHandler;
use super::implement::ImplementHandler;
use super::plan::PlanHandler;
use super::research::ResearchHandler;
use super::review::ReviewHandler;
use super::types::PhaseHandler;
use crate::pipeline::phases::Phase;

/// Create the default handler for a given phase
pub fn create_handler(phase: Phase) -> Box<dyn PhaseHandler> {
    match phase {
        Phase::Research => Box::new(ResearchHandler::new()),
        Phase::Ideation => Box::new(IdeationHandler::new()),
        Phase::Plan => Box::new(PlanHandler::new()),
        Phase::Draft => Box::new(DraftHandler::new()),
        Phase::Implement => Box::new(ImplementHandler::new()),
        Phase::Review => Box::new(ReviewHandler::new()),
        Phase::Docs => Box::new(DocsHandler::new()),
    }
}

/// Get all default handlers
pub fn default_handlers() -> Vec<Box<dyn PhaseHandler>> {
    vec![
        create_handler(Phase::Research),
        create_handler(Phase::Plan),
        create_handler(Phase::Draft),
        create_handler(Phase::Implement),
        create_handler(Phase::Review),
        create_handler(Phase::Docs),
    ]
}
