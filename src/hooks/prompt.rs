//! Prompt-Based Hook Evaluation
//!
//! Implements template-based hook evaluation that can be extended with LLM integration.
//!
//! Template format:
//! ```text
//! {{tool_name}}     - Current tool name
//! {{tool_input}}    - Tool input JSON (truncated if long)
//! {{working_dir}}   - Working directory
//! {{session_id}}    - Session ID
//! {{metadata.key}}  - Metadata field
//! ```
//!
//! Simple conditions:
//! ```toml
//! [[hooks]]
//! name = "approve-safe"
//! event = "PreToolUse"
//! template = '''
//! {% if tool_name == "Read" %} APPROVE
//! {% elif tool_name == "Glob" %} APPROVE
//! {% else %} ASK
//! {% endif %}
//! '''
//! ```
//!
//! LLM integration (future):
//! ```toml
//! [[hooks]]
//! name = "llm-review"
//! event = "PreToolUse"
//! use_llm = true
//! llm_prompt = '''
//! Evaluate if it's safe to run {{tool_name}} with input:
//! {{tool_input}}
//!
//! Respond with: APPROVE, BLOCK, or ASK
//! '''
//! ```

use regex::Regex;

use super::types::{HookExecutionContext, HookOutput};

pub struct PromptHookEvaluator {
    pub use_llm: bool,
    pub llm_prompt: Option<String>,
}

impl PromptHookEvaluator {
    pub fn new() -> Self {
        Self {
            use_llm: false,
            llm_prompt: None,
        }
    }

    pub fn with_llm(mut self, prompt: String) -> Self {
        self.use_llm = true;
        self.llm_prompt = Some(prompt);
        self
    }

    /// Evaluate a template with the given context.
    pub fn evaluate(&self, template: &str, ctx: &HookExecutionContext) -> HookOutput {
        let rendered = self.render_template(template, ctx);
        self.parse_response(&rendered)
    }

    /// Render template variables with context data.
    fn render_template(&self, template: &str, ctx: &HookExecutionContext) -> String {
        let mut result = template.to_string();

        // Tool name
        result = result.replace("{{tool_name}}", ctx.event.tool_name().unwrap_or(""));

        // Tool input (truncated for display)
        let tool_input = ctx
            .tool_input
            .as_ref()
            .map(|v| {
                let s = v.to_string();
                if s.len() > 500 {
                    format!("{}... (truncated)", &s[..500])
                } else {
                    s
                }
            })
            .unwrap_or_default();
        result = result.replace("{{tool_input}}", &tool_input);

        // Working directory
        result = result.replace("{{working_dir}}", ctx.working_dir.to_str().unwrap_or(""));

        // Session ID
        result = result.replace("{{session_id}}", ctx.session_id.as_deref().unwrap_or(""));

        // Metadata fields
        for (key, value) in &ctx.metadata {
            let placeholder = format!("{{metadata.{}}}", key);
            result = result.replace(&placeholder, value);
        }

        // Handle remaining {{...}} with defaults
        let re = Regex::new(r"\{\{(\w+(?:\.\w+)*)\}\}").unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let key = &caps[1];
                if key.starts_with("metadata.") {
                    let meta_key = key.strip_prefix("metadata.").unwrap_or(key);
                    ctx.metadata.get(meta_key).cloned().unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .to_string();

        result
    }

    /// Parse the response to determine the hook decision.
    fn parse_response(&self, response: &str) -> HookOutput {
        let response = response.trim().to_uppercase();

        // Check for explicit decision keywords
        if response.contains("APPROVE") || response.contains("ALLOW") || response.contains("YES") {
            return HookOutput::approve();
        }

        if response.contains("BLOCK") || response.contains("DENY") || response.contains("NO") {
            return HookOutput::block("Prompt hook blocked this operation");
        }

        // If LLM is configured, this would be where we'd evaluate the response
        // For now, ASK means we don't have an opinion
        if response.contains("ASK") || response.contains("UNKNOWN") {
            return HookOutput::default();
        }

        // Try to evaluate simple if/else conditions
        self.evaluate_conditions(response)
    }

    /// Evaluate simple if/elif/else conditions.
    fn evaluate_conditions(&self, content: String) -> HookOutput {
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let line = line.trim();
            if line.starts_with("APPROVE") || line.starts_with("BLOCK") || line.starts_with("ASK") {
                let decision = line.split_whitespace().next().unwrap_or("ASK");
                match decision {
                    "APPROVE" => return HookOutput::approve(),
                    "BLOCK" => return HookOutput::block("Condition blocked this operation"),
                    _ => {}
                }
            }
        }

        HookOutput::default()
    }
}

impl Default for PromptHookEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::HookDecision;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_context() -> HookExecutionContext {
        HookExecutionContext {
            event: super::super::types::HookEvent::PreToolUse {
                tool_name: "Bash".to_string(),
            },
            tool_input: Some(serde_json::json!({"command": "ls -la"})),
            tool_output: None,
            working_dir: PathBuf::from("/home/user/project"),
            session_id: Some("session-123".to_string()),
            metadata: HashMap::from([
                ("user".to_string(), "testuser".to_string()),
                ("branch".to_string(), "main".to_string()),
            ]),
        }
    }

    #[test]
    fn test_template_rendering() {
        let evaluator = PromptHookEvaluator::new();
        let ctx = test_context();

        let template = "Tool: {{tool_name}}, Dir: {{working_dir}}";
        let result = evaluator.render_template(template, &ctx);

        assert!(result.contains("Bash"));
        assert!(result.contains("/home/user/project"));
    }

    #[test]
    fn test_metadata_substitution() {
        let evaluator = PromptHookEvaluator::new();
        let ctx = test_context();

        let template = "User: {{metadata.user}}, Branch: {{metadata.branch}}";
        let result = evaluator.render_template(template, &ctx);

        assert!(result.contains("testuser"));
        assert!(result.contains("main"));
    }

    #[test]
    fn test_approve_decision() {
        let evaluator = PromptHookEvaluator::new();
        let output = evaluator.parse_response("APPROVE");
        assert_eq!(output.decision, HookDecision::Approve);
    }

    #[test]
    fn test_block_decision() {
        let evaluator = PromptHookEvaluator::new();
        let output = evaluator.parse_response("BLOCK");
        assert_eq!(output.decision, HookDecision::Block);
    }

    #[test]
    fn test_conditional_template() {
        let evaluator = PromptHookEvaluator::new();
        let ctx = test_context();

        let template = "{% if tool_name == \"Bash\" %} APPROVE\n{% else %} BLOCK\n{% endif %}";
        let output = evaluator.evaluate(template, &ctx);

        // Bash should approve
        assert_eq!(output.decision, HookDecision::Approve);
    }
}
