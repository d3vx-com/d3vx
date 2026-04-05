//! Tests for Tool Access Control
//!
//! Covers role-based tool access validation.

#[cfg(test)]
mod tests {
    use crate::tools::tool_access::{
        default_role_config, AgentRole, RoleToolConfig, RolesConfig, ToolAccessError,
        ToolAccessValidator,
    };

    // =========================================================================
    // Role Tests
    // =========================================================================

    #[test]
    fn test_agent_role_variants() {
        // Just verify variants exist and can be stringified if needed
        let _ = AgentRole::TechLead;
        let _ = AgentRole::Executor;
        let _ = AgentRole::QaEngineer;
    }

    // =========================================================================
    // Default Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_role_config_exists() {
        let config = default_role_config(AgentRole::TechLead);
        assert!(!config.allowed_tools.is_empty());
    }

    // =========================================================================
    // Tool Access Validator Tests
    // =========================================================================

    #[test]
    fn test_validator_creation() {
        let _validator = ToolAccessValidator::new();
    }

    #[test]
    fn test_validator_with_custom_config() {
        let mut config = RolesConfig::default();
        config.tech_lead = Some(RoleToolConfig {
            allowed_tools: vec!["BashTool".to_string()],
            blocked_tools: vec![],
            strict_mode: true,
        });

        let validator = ToolAccessValidator::with_config(config);
        assert!(validator.is_allowed(AgentRole::TechLead, "BashTool"));
    }

    // =========================================================================
    // Tool Permission Tests
    // =========================================================================

    #[test]
    fn test_executor_has_bash_access() {
        let validator = ToolAccessValidator::new();
        assert!(validator.is_allowed(AgentRole::Executor, "BashTool"));
    }

    #[test]
    fn test_executor_has_write_access() {
        let validator = ToolAccessValidator::new();
        assert!(validator.is_allowed(AgentRole::Executor, "WriteTool"));
    }

    #[test]
    fn test_tech_lead_no_write_access() {
        let validator = ToolAccessValidator::new();
        assert!(!validator.is_allowed(AgentRole::TechLead, "WriteTool"));
    }

    #[test]
    fn test_qa_no_write_access() {
        let validator = ToolAccessValidator::new();
        assert!(!validator.is_allowed(AgentRole::QaEngineer, "WriteTool"));
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_blocked_tool_error() {
        let validator = ToolAccessValidator::new();
        let result = validator.validate(AgentRole::TechLead, "BashTool");

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolAccessError::BlockedTool { tool, .. } => assert_eq!(tool, "BashTool"),
            _ => panic!("Expected BlockedTool error"),
        }
    }

    #[test]
    fn test_not_in_allowed_list_error() {
        let validator = ToolAccessValidator::new();
        // Executor has strict_mode true, and SpawnAgent is not in allowed list
        // Executor has strict_mode true, and UnknownTool is not in allowed/blocked lists
        let result = validator.validate(AgentRole::Executor, "UnknownTool");

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolAccessError::NotInAllowedList { tool, .. } => assert_eq!(tool, "UnknownTool"),
            _ => panic!("Expected NotInAllowedList error"),
        }
    }
}
