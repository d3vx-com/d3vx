//! Role-Based Tool Access Control
//!
//! Provides role-based tool filtering for different agent types.
//! Each role has a predefined set of allowed and blocked tools
//! to ensure proper security isolation between responsibilities.

mod types;
mod validator;

pub use types::*;
pub use validator::ToolAccessValidator;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_role_default() {
        let role = AgentRole::default();
        assert_eq!(role, AgentRole::QaEngineer);
    }

    #[test]
    fn test_default_role_config_tech_lead() {
        let config = default_role_config(AgentRole::TechLead);

        assert!(config.allowed_tools.contains(&"SpawnAgent".to_string()));
        assert!(config.allowed_tools.contains(&"ReadInbox".to_string()));
        assert!(config.blocked_tools.contains(&"BashTool".to_string()));
        assert!(config.blocked_tools.contains(&"WriteTool".to_string()));
        assert!(config.strict_mode);
    }

    #[test]
    fn test_default_role_config_qa_engineer() {
        let config = default_role_config(AgentRole::QaEngineer);

        assert!(config.allowed_tools.contains(&"BashTool".to_string()));
        assert!(config.allowed_tools.contains(&"ReadTool".to_string()));
        assert!(config.blocked_tools.contains(&"WriteTool".to_string()));
        assert!(config.blocked_tools.contains(&"EditTool".to_string()));
        assert!(config.strict_mode);
    }

    #[test]
    fn test_default_role_config_backend_developer() {
        let config = default_role_config(AgentRole::BackendDeveloper);

        assert!(config.allowed_tools.contains(&"WriteTool".to_string()));
        assert!(config.allowed_tools.contains(&"EditTool".to_string()));
        assert!(config.allowed_tools.contains(&"BashTool".to_string()));
        assert!(config.blocked_tools.is_empty());
        assert!(!config.strict_mode);
    }

    #[test]
    fn test_validator_tech_lead_cannot_bash() {
        let validator = ToolAccessValidator::new();

        let result = validator.validate(AgentRole::TechLead, "BashTool");
        assert!(result.is_err());

        match result.unwrap_err() {
            ToolAccessError::BlockedTool { tool, role } => {
                assert_eq!(tool, "BashTool");
                assert_eq!(role, AgentRole::TechLead);
            }
            _ => panic!("Expected BlockedTool error"),
        }
    }

    #[test]
    fn test_validator_qa_cannot_write() {
        let validator = ToolAccessValidator::new();

        let result = validator.validate(AgentRole::QaEngineer, "WriteTool");
        assert!(result.is_err());

        match result.unwrap_err() {
            ToolAccessError::BlockedTool { tool, role } => {
                assert_eq!(tool, "WriteTool");
                assert_eq!(role, AgentRole::QaEngineer);
            }
            _ => panic!("Expected BlockedTool error"),
        }
    }

    #[test]
    fn test_validator_backend_can_write() {
        let validator = ToolAccessValidator::new();
        let result = validator.validate(AgentRole::BackendDeveloper, "WriteTool");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_backend_can_bash() {
        let validator = ToolAccessValidator::new();
        let result = validator.validate(AgentRole::BackendDeveloper, "BashTool");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_tech_lead_can_spawn_agent() {
        let validator = ToolAccessValidator::new();
        let result = validator.validate(AgentRole::TechLead, "SpawnAgent");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_filter_tools() {
        let validator = ToolAccessValidator::new();

        let tools = vec![
            "ReadTool".to_string(),
            "WriteTool".to_string(),
            "BashTool".to_string(),
            "SpawnAgent".to_string(),
        ];

        let filtered = validator.filter_tools(AgentRole::QaEngineer, &tools);

        assert!(filtered.contains(&"ReadTool".to_string()));
        assert!(filtered.contains(&"BashTool".to_string()));
        assert!(!filtered.contains(&"WriteTool".to_string()));
        assert!(!filtered.contains(&"SpawnAgent".to_string()));
    }

    #[test]
    fn test_validator_normalize_tool_name() {
        assert_eq!(ToolAccessValidator::normalize_tool_name("Bash"), "BashTool");
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("BashTool"),
            "BashTool"
        );
        assert_eq!(ToolAccessValidator::normalize_tool_name("Read"), "ReadTool");
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("ReadTool"),
            "ReadTool"
        );
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("SpawnAgent"),
            "SpawnAgent"
        );
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("ReadInbox"),
            "ReadInbox"
        );
    }

    #[test]
    fn test_roles_config_get_config() {
        let config = RolesConfig::default();
        let tech_lead_config = config.get_config(AgentRole::TechLead);
        assert!(tech_lead_config
            .allowed_tools
            .contains(&"SpawnAgent".to_string()));
    }

    #[test]
    fn test_roles_config_with_override() {
        let custom_config = RoleToolConfig {
            allowed_tools: vec!["CustomTool".to_string()],
            blocked_tools: vec![],
            strict_mode: true,
        };

        let roles_config = RolesConfig {
            tech_lead: Some(custom_config.clone()),
            ..Default::default()
        };

        let validator = ToolAccessValidator::with_config(roles_config);

        assert!(validator.is_allowed(AgentRole::TechLead, "CustomTool"));
        assert!(!validator.is_allowed(AgentRole::TechLead, "SpawnAgent"));
    }

    #[test]
    fn test_validator_tech_lead_isolation() {
        let validator = ToolAccessValidator::new();

        assert!(validator.is_allowed(AgentRole::TechLead, "SpawnAgent"));
        assert!(validator.is_allowed(AgentRole::TechLead, "CreatePlan"));
        assert!(validator.is_allowed(AgentRole::TechLead, "ReadInbox"));

        assert!(!validator.is_allowed(AgentRole::TechLead, "BashTool"));
        assert!(!validator.is_allowed(AgentRole::TechLead, "WriteTool"));
        assert!(!validator.is_allowed(AgentRole::TechLead, "EditTool"));
    }

    #[test]
    fn test_validator_executor_isolation() {
        let validator = ToolAccessValidator::new();

        assert!(validator.is_allowed(AgentRole::Executor, "BashTool"));
        assert!(validator.is_allowed(AgentRole::Executor, "WriteTool"));
        assert!(validator.is_allowed(AgentRole::Executor, "EditTool"));

        assert!(!validator.is_allowed(AgentRole::Executor, "SpawnAgent"));
        assert!(!validator.is_allowed(AgentRole::Executor, "CreatePlan"));
        assert!(!validator.is_allowed(AgentRole::Executor, "ReadInbox"));
    }

    #[test]
    fn test_validator_normalize_new_tools() {
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("SpawnAgent"),
            "SpawnAgent"
        );
        assert_eq!(
            ToolAccessValidator::normalize_tool_name("CreatePlan"),
            "CreatePlan"
        );
    }

    #[test]
    fn test_serde_agent_role_new() {
        let role = AgentRole::Executor;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"executor\"");

        let parsed: AgentRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AgentRole::Executor);
    }
}
