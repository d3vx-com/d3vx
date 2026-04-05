//! Skills System
//!
//! On-demand skill loading and execution system.

mod discovery;
mod executor;
mod loader;
mod registry;
mod resolver;
mod types;

// Re-export discovery types
pub use discovery::{DiscoveryResult, ImportResult, SkillDiscovery};

// Re-export existing types
pub use executor::{ExecutionError, PreparedSkill, SkillExecutor};
pub use loader::SkillLoader;
pub use registry::{SkillError, SkillRegistry};
pub use resolver::{ResolverError, SkillResolver};
pub use types::{Skill, SkillContext, SkillMeta, SkillTrigger, SkillTriggerType};
