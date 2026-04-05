//! Team / Swarm Module
//!
//! Manages multi-agent team coordination, inter-agent messaging,
//! and file-based persistence for team state.

pub mod coordinator;
pub mod message_bus;
pub mod workspace;

pub use coordinator::{
    get_coordinator, get_swarm, register_swarm, unregister_swarm, MemberDescriptor, MemberStatus,
    SwarmConfig, TeamCoordinator,
};
pub use message_bus::MessageBus;
pub use workspace::{MemberEntry, TeamManifest, TeamWorkspace};
