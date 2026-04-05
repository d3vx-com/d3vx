//! Skill Executor
//!
//! Executes skills by preparing system prompts with rendered variables.

use tracing::debug;

use super::types::{Skill, SkillContext};

#[derive(Debug, Clone)]
pub struct PreparedSkill {
    pub system_prompt: String,
    pub allowed_tools: Vec<String>,
    pub skill_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Execution failed: {0}")]
    Failed(String),
}

pub struct SkillExecutor;

impl SkillExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn prepare_execution(&self, skill: &Skill, context: &SkillContext) -> PreparedSkill {
        debug!(skill = %skill.name, "Preparing skill execution");
        let system_prompt = self.render_skill(skill, context);

        PreparedSkill {
            system_prompt,
            allowed_tools: skill.tools.clone(),
            skill_name: skill.name.clone(),
        }
    }

    fn render_skill(&self, skill: &Skill, context: &SkillContext) -> String {
        let mut content = skill.content.clone();

        for (key, value) in &context.variables {
            let placeholder = format!("{{{{{}}}}}", key);
            content = content.replace(&placeholder, value);
        }

        content = content.replace("{{USER_INPUT}}", &context.user_input);
        content = content.replace("{{WORKING_DIR}}", &context.working_directory);
        if let Some(ref session_id) = context.session_id {
            content = content.replace("{{SESSION_ID}}", session_id);
        } else {
            content = content.replace("{{SESSION_ID}}", "");
        }

        content
    }

    pub fn filter_tools(&self, all_tools: &[String], allowed: &[String]) -> Vec<String> {
        if allowed.is_empty() {
            return all_tools.to_vec();
        }

        all_tools
            .iter()
            .filter(|t| {
                allowed
                    .iter()
                    .any(|a| a.as_str() == t.as_str() || t.starts_with(a.as_str()))
            })
            .cloned()
            .collect()
    }
}

impl PreparedSkill {
    pub fn new(skill_name: String, system_prompt: String, allowed_tools: Vec<String>) -> Self {
        Self {
            skill_name,
            system_prompt,
            allowed_tools,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_skill() -> Skill {
        Skill {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            triggers: vec![],
            content: r#"You are a {{ROLE}} assistant.
Working directory: {{WORKING_DIR}}
Session: {{SESSION_ID}}
User said: {{USER_INPUT}}
Custom: {{CUSTOM_VAR}}"#
                .to_string(),
            tools: vec!["Read".to_string(), "Bash".to_string()],
            source_path: None,
            enabled: true,
            depends_on: vec![],
        }
    }

    fn make_context() -> SkillContext {
        let mut ctx = SkillContext::new("Hello world", "/project");
        ctx.session_id = Some("session-123".to_string());
        ctx.set_var("ROLE", "coding");
        ctx.set_var("CUSTOM_VAR", "custom-value");
        ctx
    }

    #[test]
    fn test_prepare_execution() {
        let executor = SkillExecutor::new();
        let skill = make_skill();
        let ctx = make_context();

        let prepared = executor.prepare_execution(&skill, &ctx);

        assert_eq!(prepared.skill_name, "test-skill");
        assert!(prepared.system_prompt.contains("coding assistant"));
        assert!(prepared.system_prompt.contains("/project"));
        assert!(prepared.system_prompt.contains("session-123"));
        assert!(prepared.system_prompt.contains("Hello world"));
        assert!(prepared.system_prompt.contains("custom-value"));
        assert_eq!(prepared.allowed_tools, vec!["Read", "Bash"]);
    }

    #[test]
    fn test_filter_tools() {
        let executor = SkillExecutor::new();
        let all_tools = vec![
            "Read".to_string(),
            "Write".to_string(),
            "Bash".to_string(),
            "Glob".to_string(),
        ];

        let allowed = vec!["Read".to_string()];
        let filtered = executor.filter_tools(&all_tools, &allowed);
        assert_eq!(filtered, vec!["Read"]);

        let empty_allowed: Vec<String> = vec![];
        let all = executor.filter_tools(&all_tools, &empty_allowed);
        assert_eq!(all.len(), 4);
    }
}
