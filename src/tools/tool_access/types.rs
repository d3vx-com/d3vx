//! Role and config type definitions for tool access control

use serde::{Deserialize, Serialize};

/// Agent role enum defining different agent types with varying tool permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Technical Lead - orchestrates sub-agents, reads inbox, creates plans
    TechLead,
    /// Executor - writes code, runs bash, performs actual work
    Executor,
    /// QA Engineer - runs tests, reads files, NO write access
    #[default]
    QaEngineer,
    /// Backend Developer - legacy role, mapped to Executor/TechLead patterns
    BackendDeveloper,
}

/// Configuration for role-based tool permissions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RoleToolConfig {
    /// Tools explicitly allowed for this role
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Tools explicitly blocked for this role
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    /// Whether to use strict mode (only allowed tools, nothing else)
    #[serde(default)]
    pub strict_mode: bool,
}

/// Configuration for all roles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RolesConfig {
    pub tech_lead: Option<RoleToolConfig>,
    pub qa_engineer: Option<RoleToolConfig>,
    pub backend_developer: Option<RoleToolConfig>,
}

impl RolesConfig {
    /// Get the tool configuration for a specific role.
    pub fn get_config(&self, role: AgentRole) -> RoleToolConfig {
        let config = match role {
            AgentRole::TechLead => self.tech_lead.as_ref(),
            AgentRole::Executor => self.backend_developer.as_ref(),
            AgentRole::QaEngineer => self.qa_engineer.as_ref(),
            AgentRole::BackendDeveloper => self.backend_developer.as_ref(),
        };

        config.cloned().unwrap_or_else(|| default_role_config(role))
    }
}

/// Error type for tool access validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ToolAccessError {
    #[error("Tool '{tool}' is not authorized for role '{role:?}'")]
    UnauthorizedAccess { tool: String, role: AgentRole },
    #[error("Tool '{tool}' is explicitly blocked for role '{role:?}'")]
    BlockedTool { tool: String, role: AgentRole },
    #[error("Tool '{tool}' is not in allowed list for role '{role:?}' (strict mode)")]
    NotInAllowedList { tool: String, role: AgentRole },
}

/// Get the default tool configuration for a role.
pub fn default_role_config(role: AgentRole) -> RoleToolConfig {
    match role {
        AgentRole::TechLead => RoleToolConfig {
            allowed_tools: vec![
                "SpawnAgent".to_string(),
                "spawn_parallel_agents".to_string(),
                "CreatePlan".to_string(),
                "ReadInbox".to_string(),
                "Think".to_string(),
                "Question".to_string(),
                "WebFetch".to_string(),
                "GlobTool".to_string(),
                "GrepTool".to_string(),
                "ReadTool".to_string(),
            ],
            blocked_tools: vec![
                "BashTool".to_string(),
                "WriteTool".to_string(),
                "EditTool".to_string(),
                "MultiEditTool".to_string(),
            ],
            strict_mode: true,
        },
        AgentRole::Executor => RoleToolConfig {
            allowed_tools: vec![
                "WriteTool".to_string(),
                "EditTool".to_string(),
                "MultiEditTool".to_string(),
                "BashTool".to_string(),
                "ReadTool".to_string(),
                "GlobTool".to_string(),
                "GrepTool".to_string(),
                "Think".to_string(),
                "Question".to_string(),
                "WebFetch".to_string(),
                "TodoWrite".to_string(),
                "complete_task".to_string(),
            ],
            blocked_tools: vec![
                "SpawnAgent".to_string(),
                "spawn_parallel_agents".to_string(),
                "ReadInbox".to_string(),
                "CreatePlan".to_string(),
            ],
            strict_mode: true,
        },
        AgentRole::QaEngineer => RoleToolConfig {
            allowed_tools: vec![
                "BashTool".to_string(),
                "ReadTool".to_string(),
                "GlobTool".to_string(),
                "GrepTool".to_string(),
                "Think".to_string(),
                "Question".to_string(),
                "WebFetch".to_string(),
                "ReadInbox".to_string(),
                "complete_task".to_string(),
            ],
            blocked_tools: vec![
                "WriteTool".to_string(),
                "EditTool".to_string(),
                "MultiEditTool".to_string(),
                "SpawnAgent".to_string(),
                "spawn_parallel_agents".to_string(),
            ],
            strict_mode: true,
        },
        AgentRole::BackendDeveloper => RoleToolConfig {
            allowed_tools: vec![
                "WriteTool".to_string(),
                "EditTool".to_string(),
                "BashTool".to_string(),
                "ReadTool".to_string(),
            ],
            blocked_tools: vec![],
            strict_mode: false,
        },
    }
}
