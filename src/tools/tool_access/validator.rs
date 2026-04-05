//! Tool access validator implementation

use std::collections::HashMap;

use crate::config::types::types_claims::evaluate_claims;
use crate::config::types::ClaimsConfig;

use super::types::{default_role_config, AgentRole, RoleToolConfig, RolesConfig, ToolAccessError};

/// Validator for role-based tool access.
#[derive(Debug, Clone)]
pub struct ToolAccessValidator {
    /// Cached role configurations
    role_configs: HashMap<AgentRole, RoleToolConfig>,
    /// Optional claims-based authorization configuration
    claims_config: Option<ClaimsConfig>,
}

impl Default for ToolAccessValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolAccessValidator {
    /// Create a new validator with default role configurations.
    pub fn new() -> Self {
        let mut role_configs = HashMap::new();
        for role in [
            AgentRole::TechLead,
            AgentRole::Executor,
            AgentRole::QaEngineer,
            AgentRole::BackendDeveloper,
        ] {
            role_configs.insert(role, default_role_config(role));
        }

        Self {
            role_configs,
            claims_config: None,
        }
    }

    /// Create a validator with custom role configurations.
    pub fn with_config(config: RolesConfig) -> Self {
        let mut role_configs = HashMap::new();
        for role in [
            AgentRole::TechLead,
            AgentRole::Executor,
            AgentRole::QaEngineer,
            AgentRole::BackendDeveloper,
        ] {
            role_configs.insert(role, config.get_config(role));
        }

        Self {
            role_configs,
            claims_config: None,
        }
    }

    /// Create a validator with claims-based authorization.
    pub fn with_claims(claims: ClaimsConfig) -> Self {
        let mut role_configs = HashMap::new();
        for role in [
            AgentRole::TechLead,
            AgentRole::Executor,
            AgentRole::QaEngineer,
            AgentRole::BackendDeveloper,
        ] {
            role_configs.insert(role, default_role_config(role));
        }

        Self {
            role_configs,
            claims_config: Some(claims),
        }
    }

    /// Set or update the claims configuration.
    pub fn set_claims(&mut self, claims: Option<ClaimsConfig>) {
        self.claims_config = claims;
    }

    /// Check if a claim matches for a given role and action.
    ///
    /// Returns:
    /// - `Some(true)` if a matching claim grants access
    /// - `Some(false)` if a matching claim denies access
    /// - `None` if no claims are configured or no claim matches
    pub fn check_claim(&self, role: &str, action: &str) -> Option<bool> {
        let claims_config = self.claims_config.as_ref()?;
        let claims = claims_config.roles.get(role)?;
        evaluate_claims(claims, action)
    }

    /// Validate if a tool can be used by a specific role.
    pub fn validate(&self, role: AgentRole, tool_name: &str) -> Result<(), ToolAccessError> {
        // Check claims-based authorization first, if configured.
        // Map the AgentRole to its string name for claims lookup.
        let role_name = match role {
            AgentRole::TechLead => "tech_lead",
            AgentRole::Executor => "executor",
            AgentRole::QaEngineer => "qa_engineer",
            AgentRole::BackendDeveloper => "backend_developer",
        };

        let action = format!("tools:{}", Self::normalize_tool_name(tool_name));

        if let Some(granted) = self.check_claim(role_name, &action) {
            if granted {
                return Ok(());
            }
            return Err(ToolAccessError::BlockedTool {
                tool: tool_name.to_string(),
                role,
            });
        }

        // Fall back to existing role-based configuration
        let config = self
            .role_configs
            .get(&role)
            .cloned()
            .unwrap_or_else(|| default_role_config(role));

        let normalized_name = Self::normalize_tool_name(tool_name);

        if Self::tool_matches_list(&normalized_name, &config.blocked_tools) {
            return Err(ToolAccessError::BlockedTool {
                tool: tool_name.to_string(),
                role,
            });
        }

        if config.strict_mode {
            if !Self::tool_matches_list(&normalized_name, &config.allowed_tools) {
                return Err(ToolAccessError::NotInAllowedList {
                    tool: tool_name.to_string(),
                    role,
                });
            }
        }

        Ok(())
    }

    /// Check if a tool is allowed for a specific role.
    pub fn is_allowed(&self, role: AgentRole, tool_name: &str) -> bool {
        self.validate(role, tool_name).is_ok()
    }

    /// Filter a list of tool names to only include those allowed for a role.
    pub fn filter_tools(&self, role: AgentRole, tool_names: &[String]) -> Vec<String> {
        tool_names
            .iter()
            .filter(|name| self.is_allowed(role, name))
            .cloned()
            .collect()
    }

    /// Get all allowed tool names for a role.
    pub fn get_allowed_tools(&self, role: AgentRole) -> Vec<String> {
        let config = self
            .role_configs
            .get(&role)
            .cloned()
            .unwrap_or_else(|| default_role_config(role));
        config.allowed_tools.clone()
    }

    /// Get all blocked tool names for a role.
    pub fn get_blocked_tools(&self, role: AgentRole) -> Vec<String> {
        let config = self
            .role_configs
            .get(&role)
            .cloned()
            .unwrap_or_else(|| default_role_config(role));
        config.blocked_tools.clone()
    }

    /// Normalize tool name to handle variations.
    pub fn normalize_tool_name(name: &str) -> String {
        let name = name.trim();

        if !name.ends_with("Tool") && !name.ends_with("Inbox") {
            let tool_variants = [
                "Bash",
                "Read",
                "Write",
                "Edit",
                "Glob",
                "Grep",
                "Think",
                "Question",
                "WebFetch",
                "MultiEdit",
                "Todo",
            ];
            if tool_variants.contains(&name) {
                return format!("{}Tool", name);
            }
        }

        name.to_string()
    }

    /// Check if a tool name matches any pattern in a list.
    fn tool_matches_list(tool_name: &str, list: &[String]) -> bool {
        let normalized = Self::normalize_tool_name(tool_name);

        for pattern in list {
            let normalized_pattern = Self::normalize_tool_name(pattern);

            if normalized == normalized_pattern {
                return true;
            }

            if normalized == pattern.trim() || normalized_pattern == tool_name.trim() {
                return true;
            }
        }

        false
    }

    /// Update role configuration at runtime.
    pub fn update_role_config(&mut self, role: AgentRole, config: RoleToolConfig) {
        self.role_configs.insert(role, config);
    }
}
