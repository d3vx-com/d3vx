//! Skill Loader
//!
//! Parses SKILL.md files and loads skill definitions.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::warn;

use super::types::{Skill, SkillMeta, SkillTrigger};

/// Frontmatter parsed from a SKILL.md file
#[derive(Debug, serde::Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default)]
    triggers: Vec<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    depends_on: Vec<String>,
}

/// Loader for SKILL.md files
pub struct SkillLoader {
    search_paths: Vec<PathBuf>,
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillLoader {
    /// Create a new skill loader
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    /// Add a search path for skills
    pub fn add_search_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    /// Load a skill from a specific file
    pub fn load_skill(&self, path: &Path) -> Result<Skill> {
        let content = std::fs::read_to_string(path)?;
        self.parse_skill(&content.trim(), Some(path))
    }

    /// Load a skill by name (searches in configured paths)
    pub fn load_skill_by_name(&self, name: &str) -> Result<Option<Skill>> {
        for path in &self.search_paths {
            let skill_path = path.join(name).join("SKILL.md");
            if skill_path.exists() {
                return Ok(Some(self.load_skill(&skill_path)?));
            }

            let direct_path = path.join(format!("{}.md", name));
            if direct_path.exists() {
                return Ok(Some(self.load_skill(&direct_path)?));
            }

            let skill_dir = path.join(name);
            if skill_dir.is_dir() {
                let skill_file = skill_dir.join("SKILL.md");
                if skill_file.exists() {
                    return Ok(Some(self.load_skill(&skill_file)?));
                }
            }
        }
        Ok(None)
    }

    /// List available skills in search paths
    pub fn list_skills(&self) -> Vec<SkillMeta> {
        let mut skills = Vec::new();

        for path in &self.search_paths {
            if !path.exists() {
                continue;
            }

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        if let Ok(skill) = self.load_skill(path) {
                            skills.push(self.to_meta(&skill));
                        }
                    }
                }
            } else if path.is_dir() {
                // Check for SKILL.md in directory
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if let Ok(skill) = self.load_skill(&skill_file) {
                        skills.push(self.to_meta(&skill));
                    }
                    continue;
                }

                // Check subdirectories
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            let inner_skill = entry_path.join("SKILL.md");
                            if inner_skill.exists() {
                                if let Ok(skill) = self.load_skill(&inner_skill) {
                                    skills.push(self.to_meta(&skill));
                                }
                            }
                        }
                    }
                }
            }
        }

        skills
    }

    /// Parse a SKILL.md file content
    fn parse_skill(&self, content: &str, source_path: Option<&Path>) -> Result<Skill> {
        let (frontmatter, body) = self.parse_frontmatter(content)?;

        let triggers = self.parse_triggers(&frontmatter)?;

        let name = frontmatter.name.clone();
        let description = frontmatter.description.clone();
        let tools = frontmatter.tools.clone();
        let enabled = frontmatter.enabled;
        let depends_on = frontmatter.depends_on.clone();

        Ok(Skill {
            name,
            description,
            triggers,
            content: body.trim().to_string(),
            tools,
            source_path: source_path.map(|p| p.to_string_lossy().to_string()),
            enabled,
            depends_on,
        })
    }

    /// Parse YAML frontmatter from SKILL.md content
    fn parse_frontmatter(&self, content: &str) -> Result<(SkillFrontmatter, String)> {
        let content = content.trim();

        if content.starts_with("---") {
            let end = content[3..]
                .find("---")
                .ok_or_else(|| anyhow::anyhow!("Unclosed frontmatter"))?;

            let yaml_content = &content[3..3 + end];
            let body = &content[3 + end + 3..];

            let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_content)
                .map_err(|e| anyhow::anyhow!("Failed to parse frontmatter: {}", e))?;

            Ok((frontmatter, body.to_string()))
        } else {
            // No frontmatter, treat entire content as skill body
            let frontmatter = SkillFrontmatter {
                name: "Unnamed Skill".to_string(),
                description: "No description".to_string(),
                trigger: None,
                triggers: Vec::new(),
                tools: Vec::new(),
                enabled: true,
                depends_on: Vec::new(),
            };
            Ok((frontmatter, content.to_string()))
        }
    }

    /// Parse triggers from frontmatter
    fn parse_triggers(&self, fm: &SkillFrontmatter) -> Result<Vec<SkillTrigger>> {
        let mut triggers = Vec::new();

        // Parse single trigger
        if let Some(ref trigger) = fm.trigger {
            triggers.push(self.parse_trigger_string(trigger)?);
        }

        // Parse multiple triggers
        for trigger in &fm.triggers {
            triggers.push(self.parse_trigger_string(trigger)?);
        }

        Ok(triggers)
    }

    /// Parse a trigger string like "command:/review" or "keyword:review"
    fn parse_trigger_string(&self, s: &str) -> Result<SkillTrigger> {
        let s = s.trim();

        if let Some((prefix, pattern)) = s.split_once(':') {
            let prefix = prefix.trim().to_lowercase();
            let pattern = pattern.trim();

            match prefix.as_str() {
                "command" | "cmd" => Ok(SkillTrigger::command(pattern)),
                "keyword" | "word" => Ok(SkillTrigger::keyword(pattern)),
                "tool" | "toolcall" => Ok(SkillTrigger::tool_call(pattern)),
                "mention" | "@" => Ok(SkillTrigger::mention(pattern)),
                _ => {
                    // Default to command trigger
                    warn!("Unknown trigger prefix '{}', treating as command", prefix);
                    Ok(SkillTrigger::command(s))
                }
            }
        } else {
            // No prefix, default to command trigger
            Ok(SkillTrigger::command(s))
        }
    }

    /// Convert skill to metadata
    fn to_meta(&self, skill: &Skill) -> SkillMeta {
        SkillMeta {
            name: skill.name.clone(),
            description: skill.description.clone(),
            triggers: skill
                .triggers
                .iter()
                .map(|t| t.trigger_type.clone())
                .collect(),
            tool_count: skill.tools.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::types::SkillTriggerType;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: review
description: Review code changes
trigger: command:/review
tools:
  - Read
  - Bash
---

# Review Agent

You are a code reviewer.
"#;

        let loader = SkillLoader::new();
        let result = loader.parse_frontmatter(content);
        assert!(result.is_ok());

        let (fm, body) = result.unwrap();
        assert_eq!(fm.name, "review");
        assert_eq!(fm.description, "Review code changes");
        assert_eq!(fm.trigger.as_deref(), Some("command:/review"));
        assert_eq!(fm.tools, vec!["Read", "Bash"]);
        assert!(body.contains("Review Agent"));
    }

    #[test]
    fn test_parse_trigger_string() {
        let loader = SkillLoader::new();

        let trigger = loader.parse_trigger_string("command:/review").unwrap();
        assert_eq!(trigger.trigger_type, SkillTriggerType::Command);
        assert_eq!(trigger.pattern, "/review");

        let trigger = loader.parse_trigger_string("keyword:test").unwrap();
        assert_eq!(trigger.trigger_type, SkillTriggerType::Keyword);

        let trigger = loader.parse_trigger_string("/legacy").unwrap();
        assert_eq!(trigger.trigger_type, SkillTriggerType::Command);
        assert_eq!(trigger.pattern, "/legacy");
    }

    #[test]
    fn test_trigger_matches() {
        let trigger = SkillTrigger::command("review");

        assert!(trigger.matches("/review"));
        assert!(trigger.matches("review"));
        assert!(!trigger.matches("/other"));

        let keyword = SkillTrigger::keyword("review");
        assert!(keyword.matches("Please review this code"));
        assert!(keyword.matches("REVIEW"));
    }
}
