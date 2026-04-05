//! Skill type definitions

use serde::{Deserialize, Serialize};

/// Skill definition loaded from SKILL.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique skill identifier
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Triggers that activate this skill
    pub triggers: Vec<SkillTrigger>,
    /// The skill content/system prompt
    pub content: String,
    /// Tools available to this skill
    pub tools: Vec<String>,
    /// File path where this skill was loaded from
    pub source_path: Option<String>,
    /// Whether this skill is enabled
    pub enabled: bool,
    /// Skills this skill depends on
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Metadata for skill discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub triggers: Vec<SkillTriggerType>,
    pub tool_count: usize,
}

/// Trigger type for skill activation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillTriggerType {
    /// Triggered by a slash command (e.g., /review)
    Command,
    /// Triggered by a keyword in the prompt
    Keyword,
    /// Triggered when a specific tool is called
    ToolCall,
    /// Triggered automatically when skill is mentioned
    Mention,
}

/// Skill trigger definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTrigger {
    #[serde(rename = "type")]
    pub trigger_type: SkillTriggerType,
    pub pattern: String,
}

impl SkillTrigger {
    /// Create a command trigger
    pub fn command(cmd: impl Into<String>) -> Self {
        Self {
            trigger_type: SkillTriggerType::Command,
            pattern: cmd.into(),
        }
    }

    /// Create a keyword trigger
    pub fn keyword(keyword: impl Into<String>) -> Self {
        Self {
            trigger_type: SkillTriggerType::Keyword,
            pattern: keyword.into(),
        }
    }

    /// Create a tool call trigger
    pub fn tool_call(tool: impl Into<String>) -> Self {
        Self {
            trigger_type: SkillTriggerType::ToolCall,
            pattern: tool.into(),
        }
    }

    /// Create a mention trigger
    pub fn mention(name: impl Into<String>) -> Self {
        Self {
            trigger_type: SkillTriggerType::Mention,
            pattern: name.into(),
        }
    }

    /// Check if this trigger matches the given input
    pub fn matches(&self, input: &str) -> bool {
        match self.trigger_type {
            SkillTriggerType::Command => {
                input.trim_start_matches('/') == self.pattern
                    || input.starts_with(&format!("/{}", self.pattern))
            }
            SkillTriggerType::Keyword => {
                input.to_lowercase().contains(&self.pattern.to_lowercase())
            }
            SkillTriggerType::ToolCall => input == self.pattern,
            SkillTriggerType::Mention => input
                .to_lowercase()
                .contains(&format!("@{}", self.pattern).to_lowercase()),
        }
    }
}

/// Skill execution context
#[derive(Debug, Clone)]
pub struct SkillContext {
    pub user_input: String,
    pub working_directory: String,
    pub session_id: Option<String>,
    pub variables: std::collections::HashMap<String, String>,
}

impl SkillContext {
    pub fn new(user_input: impl Into<String>, working_directory: impl Into<String>) -> Self {
        Self {
            user_input: user_input.into(),
            working_directory: working_directory.into(),
            session_id: None,
            variables: std::collections::HashMap::new(),
        }
    }

    /// Get a variable with optional default
    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    /// Set a variable
    pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }
}

impl Default for SkillContext {
    fn default() -> Self {
        Self {
            user_input: String::new(),
            working_directory: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
            session_id: None,
            variables: std::collections::HashMap::new(),
        }
    }
}
