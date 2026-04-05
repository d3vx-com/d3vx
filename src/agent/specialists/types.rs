//! Specialized SDLC Agent Roles
//!
//! Defines specialized agent roles that can be spawned for specific tasks.
//! The orchestrator agent decides which specialized agents to use based on task context.
//! NO keyword-based detection - the AI decides based on task analysis.

use serde::{Deserialize, Serialize};

use crate::tools::AgentRole;

#[derive(Debug, Clone)]
pub struct SpecialistProfile {
    pub agent_type: AgentType,
    pub display_name: &'static str,
    pub specialist_role_label: &'static str,
    pub system_prompt: &'static str,
    pub recommended_role: AgentRole,
    pub suggested_skill_slug: Option<&'static str>,
    pub review_focus: &'static [&'static str],
}

/// Specialized agent types for SDLC tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// General purpose agent - can handle any task
    General,
    /// Backend development (APIs, databases, business logic)
    Backend,
    /// Frontend development (UI, components, styling)
    Frontend,
    /// Testing (unit, integration, e2e)
    Testing,
    /// Documentation (README, API docs, guides)
    Documentation,
    /// DevOps (CI/CD, infrastructure, deployment)
    DevOps,
    /// Security (audit, vulnerability assessment)
    Security,
    /// Code review and quality assurance
    Review,
    /// Data engineering (pipelines, ETL, analytics)
    Data,
    /// Mobile development (iOS, Android, cross-platform)
    Mobile,
    /// Read-only codebase exploration and search
    Explore,
    /// Read-only architecture planning
    Plan,
    /// Coordinated swarm teammate
    Teammate,
}
