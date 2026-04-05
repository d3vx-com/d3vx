//! Agent Event Handling Module
//!
//! Organized into modular components:
//! - `agent_event` - Main agent event dispatch
//! - `agent_start` - Agent start bookkeeping and inbox handling
//! - `workspace_event` - Workspace-scoped agent event handling
//! - `workspace_done` - Workspace agent completion handling
//! - `spawn_parallel` - Parallel event handling and single agent spawning
//! - `state_fields` - State field application helper
//! - `batch_spawn` - Batch task scheduling and spawning
//! - `batch_child_update` - Batch child status updates
//! - `batch_extract` - Result extraction from child tasks
//! - `batch_helpers` - Batch utility methods and types
//! - `batch_synthesis` - Synthesis report building
//! - `evaluation` - Parallel child evaluation heuristics
//! - `extraction` - Text extraction utilities
//! - `coordination` - Inter-agent coordination messages

pub mod agent_event;
pub mod agent_start;
pub mod batch_child_update;
pub mod batch_extract;
pub mod batch_helpers;
pub mod batch_spawn;
pub mod batch_synthesis;
pub mod coordination;
pub mod evaluation;
pub mod extraction;
pub mod spawn_parallel;
pub mod state_fields;
pub mod workspace_done;
pub mod workspace_event;
