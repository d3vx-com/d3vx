use super::prompts::*;
use super::types::{AgentType, SpecialistProfile};
use crate::tools::AgentRole;

impl AgentType {
    /// Get the system prompt addition for this agent type
    pub fn system_prompt_addition(&self) -> &'static str {
        match self {
            AgentType::General => "",
            AgentType::Backend => SPECIALIST_BACKEND,
            AgentType::Frontend => SPECIALIST_FRONTEND,
            AgentType::Testing => SPECIALIST_TESTING,
            AgentType::Documentation => SPECIALIST_DOCUMENTATION,
            AgentType::DevOps => SPECIALIST_DEVOPS,
            AgentType::Security => SPECIALIST_SECURITY,
            AgentType::Review => SPECIALIST_REVIEW,
            AgentType::Data => SPECIALIST_DATA,
            AgentType::Mobile => SPECIALIST_MOBILE,
            AgentType::Explore => SPECIALIST_EXPLORE,
            AgentType::Plan => SPECIALIST_PLAN,
            AgentType::Teammate => TEAMMATE_SYSTEM,
        }
    }

    /// Get the display name for this agent type
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::General => "Agent",
            AgentType::Backend => "Backend Dev",
            AgentType::Frontend => "Frontend Dev",
            AgentType::Testing => "QA Engineer",
            AgentType::Documentation => "Tech Writer",
            AgentType::DevOps => "DevOps Engineer",
            AgentType::Security => "Security Engineer",
            AgentType::Review => "Code Reviewer",
            AgentType::Data => "Data Engineer",
            AgentType::Mobile => "Mobile Dev",
            AgentType::Explore => "Explorer",
            AgentType::Plan => "Architect",
            AgentType::Teammate => "Swarm Mate",
        }
    }

    /// Human-readable specialist role label for UI and orchestration summaries.
    pub fn specialist_role_label(&self) -> &'static str {
        match self {
            AgentType::General => "Executor",
            AgentType::Backend => "Backend Specialist",
            AgentType::Frontend => "Frontend Specialist",
            AgentType::Testing => "Test Specialist",
            AgentType::Documentation => "Documentation Specialist",
            AgentType::DevOps => "DevOps Specialist",
            AgentType::Security => "Security Reviewer",
            AgentType::Review => "Code Reviewer",
            AgentType::Data => "Data Specialist",
            AgentType::Mobile => "Mobile Specialist",
            AgentType::Explore => "Explore Specialist",
            AgentType::Plan => "Plan Specialist",
            AgentType::Teammate => "Swarm Teammate",
        }
    }

    /// Get the icon for this agent type
    pub fn icon(&self) -> &'static str {
        match self {
            AgentType::General => "🤖",
            AgentType::Backend => "⚙️",
            AgentType::Frontend => "🎨",
            AgentType::Testing => "🧪",
            AgentType::Documentation => "📝",
            AgentType::DevOps => "🚀",
            AgentType::Security => "🔒",
            AgentType::Review => "🔍",
            AgentType::Data => "📊",
            AgentType::Mobile => "📱",
            AgentType::Explore => "🔍",
            AgentType::Plan => "📋",
            AgentType::Teammate => "🐝",
        }
    }

    /// Get the priority for this agent type (higher = more specialized)
    pub fn priority(&self) -> u8 {
        match self {
            AgentType::General => 0,
            AgentType::Backend => 3,
            AgentType::Frontend => 3,
            AgentType::Testing => 4,
            AgentType::Documentation => 2,
            AgentType::DevOps => 4,
            AgentType::Security => 5,
            AgentType::Review => 4,
            AgentType::Data => 3,
            AgentType::Mobile => 3,
            AgentType::Explore => 2,
            AgentType::Plan => 3,
            AgentType::Teammate => 1,
        }
    }

    pub fn profile(&self) -> SpecialistProfile {
        let (recommended_role, suggested_skill_slug, review_focus) = match self {
            AgentType::General => (
                AgentRole::Executor,
                None,
                &[
                    "complete the requested task safely",
                    "report concrete outcomes",
                ][..],
            ),
            AgentType::Backend => (
                AgentRole::Executor,
                Some("backend"),
                &[
                    "api correctness",
                    "data model integrity",
                    "service boundaries",
                ][..],
            ),
            AgentType::Frontend => (
                AgentRole::Executor,
                Some("frontend"),
                &[
                    "component quality",
                    "states and accessibility",
                    "ui consistency",
                ][..],
            ),
            AgentType::Testing => (
                AgentRole::QaEngineer,
                Some("testing"),
                &[
                    "regression coverage",
                    "failure reproduction",
                    "test signal quality",
                ][..],
            ),
            AgentType::Documentation => (
                AgentRole::Executor,
                Some("documentation"),
                &[
                    "onboarding clarity",
                    "examples",
                    "documentation completeness",
                ][..],
            ),
            AgentType::DevOps => (
                AgentRole::Executor,
                Some("devops"),
                &[
                    "pipeline reliability",
                    "deployment safety",
                    "operational clarity",
                ][..],
            ),
            AgentType::Security => (
                AgentRole::QaEngineer,
                Some("security"),
                &["risk hotspots", "input handling", "auth and secret safety"][..],
            ),
            AgentType::Review => (
                AgentRole::QaEngineer,
                Some("review"),
                &["correctness", "regression risk", "merge readiness"][..],
            ),
            AgentType::Data => (
                AgentRole::Executor,
                Some("data"),
                &["schema correctness", "pipeline integrity", "data quality"][..],
            ),
            AgentType::Mobile => (
                AgentRole::Executor,
                Some("mobile"),
                &[
                    "platform consistency",
                    "runtime stability",
                    "device constraints",
                ][..],
            ),
            AgentType::Explore => (
                AgentRole::QaEngineer,
                Some("explore"),
                &["search completeness", "file coverage", "accuracy"][..],
            ),
            AgentType::Plan => (
                AgentRole::QaEngineer,
                Some("plan"),
                &[
                    "architecture soundness",
                    "implementation feasibility",
                    "edge cases",
                ][..],
            ),
            AgentType::Teammate => (
                AgentRole::Executor,
                Some("teammate"),
                &["task completion", "communication", "coordination"][..],
            ),
        };

        SpecialistProfile {
            agent_type: *self,
            display_name: self.display_name(),
            specialist_role_label: self.specialist_role_label(),
            system_prompt: self.system_prompt_addition(),
            recommended_role,
            suggested_skill_slug,
            review_focus,
        }
    }

    pub fn resolve_project_context(&self, working_dir: &str) -> Option<String> {
        let slug = self.profile().suggested_skill_slug?;
        let path = std::path::Path::new(working_dir)
            .join(".d3vx")
            .join("skills")
            .join(format!("{slug}.md"));
        let content = std::fs::read_to_string(path).ok()?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() > 4000 {
            Some(format!(
                "Project-specific specialist context ({slug}):\n{}",
                &trimmed[..4000]
            ))
        } else {
            Some(format!(
                "Project-specific specialist context ({slug}):\n{trimmed}"
            ))
        }
    }
}

impl Default for AgentType {
    fn default() -> Self {
        AgentType::General
    }
}

/// All available specialist agent types
pub const SPECIALIST_AGENT_TYPES: &[AgentType] = &[
    AgentType::Backend,
    AgentType::Frontend,
    AgentType::Testing,
    AgentType::Documentation,
    AgentType::DevOps,
    AgentType::Security,
    AgentType::Review,
    AgentType::Data,
    AgentType::Mobile,
    AgentType::Explore,
    AgentType::Plan,
    AgentType::Teammate,
];

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
