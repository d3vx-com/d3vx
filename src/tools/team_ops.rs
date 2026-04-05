//! Team operations tools: FormSwarmTool and DisbandSwarmTool.

use crate::agent::specialists::AgentType;
use crate::team::{
    get_coordinator, register_swarm, unregister_swarm, MessageBus, SwarmConfig, TeamCoordinator,
    TeamWorkspace,
};
use crate::tools::tool_access::AgentRole;
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_agent_type(s: &str) -> Option<AgentType> {
    match s {
        "general" => Some(AgentType::General),
        "backend" => Some(AgentType::Backend),
        "frontend" => Some(AgentType::Frontend),
        "testing" => Some(AgentType::Testing),
        "documentation" => Some(AgentType::Documentation),
        "devops" => Some(AgentType::DevOps),
        "security" => Some(AgentType::Security),
        "review" => Some(AgentType::Review),
        "data" => Some(AgentType::Data),
        "mobile" => Some(AgentType::Mobile),
        "explore" => Some(AgentType::Explore),
        "plan" => Some(AgentType::Plan),
        "teammate" => Some(AgentType::Teammate),
        _ => None,
    }
}

fn parse_agent_role(s: &str) -> Option<AgentRole> {
    match s {
        "lead" => Some(AgentRole::TechLead),
        "executor" => Some(AgentRole::Executor),
        "qa_engineer" => Some(AgentRole::QaEngineer),
        "planner" => Some(AgentRole::TechLead),
        "reviewer" => Some(AgentRole::QaEngineer),
        _ => None,
    }
}

/// Extract a required string field or return a ToolResult error.
fn str_field<'a>(
    input: &'a serde_json::Value,
    field: &str,
) -> std::result::Result<&'a str, ToolResult> {
    input
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolResult::error(format!("Missing required field: '{}'", field)))
}

// ---------------------------------------------------------------------------
// FormSwarmTool
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct FormSwarmTool;

impl FormSwarmTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FormSwarmTool {
    fn name(&self) -> String {
        "form_swarm".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Create a new coordinated swarm with the given members.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique name for this swarm" },
                    "description": { "type": "string", "description": "What this swarm is working on" },
                    "members": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "call_sign": { "type": "string" },
                                "agent_type": { "type": "string", "enum": [
                                    "general","backend","frontend","testing","documentation",
                                    "devops","security","review","data","mobile","explore","plan","teammate"
                                ] },
                                "role": { "type": "string", "enum": ["lead","executor","qa_engineer","planner","reviewer"] }
                            },
                            "required": ["call_sign", "agent_type", "role"]
                        }
                    }
                },
                "required": ["name", "description"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let name = match str_field(&input, "name") {
            Ok(s) => s.to_string(),
            Err(e) => return e,
        };
        let description = match str_field(&input, "description") {
            Ok(s) => s.to_string(),
            Err(e) => return e,
        };

        if get_coordinator(&name).is_some() {
            return ToolResult::error(format!("A swarm named '{}' already exists", name));
        }

        let config = SwarmConfig {
            name: name.clone(),
            description: description.clone(),
            base_cwd: context.cwd.clone(),
            max_members: 5,
        };
        let coordinator = TeamCoordinator::new(config, MessageBus::new());

        let mut enrolled: Vec<serde_json::Value> = Vec::new();
        if let Some(arr) = input.get("members").and_then(|v| v.as_array()) {
            for member in arr {
                let cs = match str_field(member, "call_sign") {
                    Ok(s) => s.to_string(),
                    Err(e) => return e,
                };
                let at_str = match member.get("agent_type").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolResult::error("Each member must have an 'agent_type'"),
                };
                let role_str = match member.get("role").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolResult::error("Each member must have a 'role'"),
                };
                let agent_type = match parse_agent_type(at_str) {
                    Some(at) => at,
                    None => {
                        return ToolResult::error(format!(
                            "Invalid agent_type '{}' for member '{}'",
                            at_str, cs
                        ))
                    }
                };
                let role = match parse_agent_role(role_str) {
                    Some(r) => r,
                    None => {
                        return ToolResult::error(format!(
                            "Invalid role '{}' for member '{}'",
                            role_str, cs
                        ))
                    }
                };

                match coordinator
                    .enroll_member(cs.clone(), agent_type, role)
                    .await
                {
                    Ok(desc) => {
                        enrolled.push(
                            json!({ "call_sign": desc.call_sign, "agent_id": desc.agent_id }),
                        );
                        if role_str == "lead" {
                            coordinator.set_lead(&cs).await;
                        }
                    }
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Failed to enroll member '{}': {}",
                            cs, e
                        ))
                    }
                }
            }
        }

        let workspace = TeamWorkspace::new(&context.cwd, &name);
        if let Err(e) = coordinator.persist_to_workspace(&workspace).await {
            return ToolResult::error(format!("Failed to persist workspace: {}", e));
        }
        register_swarm(&name, Arc::new(coordinator));

        ToolResult::success(
            json!({
                "status": "formed", "swarm": name, "description": description,
                "member_count": enrolled.len(), "members": enrolled,
            })
            .to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// DisbandSwarmTool
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct DisbandSwarmTool;

impl DisbandSwarmTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for DisbandSwarmTool {
    fn name(&self) -> String {
        "disband_swarm".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Disband an existing swarm, cleaning up all resources.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string", "description": "Name of the swarm to disband" } },
                "required": ["name"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let name = match str_field(&input, "name") {
            Ok(s) => s.to_string(),
            Err(e) => return e,
        };
        let coordinator = match get_coordinator(&name) {
            Some(c) => c,
            None => return ToolResult::error(format!("No active swarm named '{}' found", name)),
        };

        coordinator.deactivate().await;
        let members = coordinator.list_members().await;
        for m in &members {
            coordinator.bus().unregister(&m.call_sign).await;
        }

        let workspace = TeamWorkspace::new(&context.cwd, &name);
        if let Err(e) = workspace.delete() {
            return ToolResult::error(format!("Failed to delete workspace: {}", e));
        }
        unregister_swarm(&name);

        ToolResult::success(
            json!({ "status": "disbanded", "swarm": name, "removed_members": members.len() })
                .to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_context() -> ToolContext {
        ToolContext {
            cwd: "/tmp".into(),
            env: HashMap::new(),
            trust_mode: false,
            session_id: Some("test".into()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            bash_blocklist: vec![],
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
            swarm_membership: None,
        }
    }

    #[test]
    fn form_swarm_name() {
        assert_eq!(FormSwarmTool::new().name(), "form_swarm");
    }
    #[test]
    fn disband_swarm_name() {
        assert_eq!(DisbandSwarmTool::new().name(), "disband_swarm");
    }

    #[tokio::test]
    async fn form_swarm_creates_and_registers() {
        let swarm_name = "test-form-create";
        unregister_swarm(swarm_name);
        let input = json!({ "name": swarm_name, "description": "test", "members": [
            { "call_sign": "lead-1", "agent_type": "general", "role": "lead" },
            { "call_sign": "exec-1", "agent_type": "backend", "role": "executor" }
        ]});
        let result = FormSwarmTool::new().execute(input, &make_context()).await;
        assert!(!result.is_error, "{}", result.content);
        let body: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(body["status"], "formed");
        assert_eq!(body["member_count"], 2);
        assert!(get_coordinator(swarm_name).is_some());
        unregister_swarm(swarm_name);
    }

    #[tokio::test]
    async fn form_swarm_rejects_duplicate_name() {
        let swarm_name = "test-dup-swarm";
        unregister_swarm(swarm_name);
        let input = json!({ "name": swarm_name, "description": "first" });
        let _ = FormSwarmTool::new().execute(input, &make_context()).await;
        let result = FormSwarmTool::new()
            .execute(
                json!({ "name": swarm_name, "description": "second" }),
                &make_context(),
            )
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("already exists"));
        unregister_swarm(swarm_name);
    }

    #[tokio::test]
    async fn disband_swarm_removes_from_registry() {
        let swarm_name = "test-disband";
        unregister_swarm(swarm_name);
        let input = json!({ "name": swarm_name, "description": "to be disbanded", "members": [
            { "call_sign": "alpha", "agent_type": "general", "role": "lead" }
        ]});
        let _ = FormSwarmTool::new().execute(input, &make_context()).await;
        assert!(get_coordinator(swarm_name).is_some());
        let result = DisbandSwarmTool::new()
            .execute(json!({ "name": swarm_name }), &make_context())
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result.content).unwrap()["status"],
            "disbanded"
        );
        assert!(get_coordinator(swarm_name).is_none());
    }

    #[tokio::test]
    async fn disband_swarm_errors_on_nonexistent() {
        let swarm_name = "no-such-swarm";
        unregister_swarm(swarm_name);
        let result = DisbandSwarmTool::new()
            .execute(json!({ "name": swarm_name }), &make_context())
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("No active swarm"));
    }

    #[test]
    fn parse_agent_type_variants() {
        assert!(matches!(
            parse_agent_type("general"),
            Some(AgentType::General)
        ));
        assert!(matches!(
            parse_agent_type("backend"),
            Some(AgentType::Backend)
        ));
        assert!(matches!(
            parse_agent_type("teammate"),
            Some(AgentType::Teammate)
        ));
        assert!(parse_agent_type("unknown").is_none());
    }

    #[test]
    fn parse_agent_role_variants() {
        assert!(matches!(
            parse_agent_role("lead"),
            Some(AgentRole::TechLead)
        ));
        assert!(matches!(
            parse_agent_role("executor"),
            Some(AgentRole::Executor)
        ));
        assert!(matches!(
            parse_agent_role("qa_engineer"),
            Some(AgentRole::QaEngineer)
        ));
        assert_eq!(parse_agent_role("planner"), Some(AgentRole::TechLead));
        assert_eq!(parse_agent_role("reviewer"), Some(AgentRole::QaEngineer));
        assert!(parse_agent_role("unknown").is_none());
    }
}
