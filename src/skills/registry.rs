//! Skill Registry
//!
//! Central registry for managing and discovering skills.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::executor::PreparedSkill;
use super::loader::SkillLoader;
use super::resolver::SkillResolver;
use super::types::{Skill, SkillContext, SkillMeta, SkillTriggerType};

/// Central registry for skills
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, Arc<Skill>>>>,
    loader: SkillLoader,
    resolver: SkillResolver,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
            loader: SkillLoader::new(),
            resolver: SkillResolver::new(),
        }
    }

    pub fn add_search_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        let path_buf = if path.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path.trim_start_matches("~/")))
                .unwrap_or_else(|| std::path::PathBuf::from(&path))
        } else {
            std::path::PathBuf::from(&path)
        };
        self.loader = self.loader.add_search_path(path_buf);
        self
    }

    pub async fn register(&self, skill: Arc<Skill>) {
        let name = skill.name.clone();
        self.skills.write().await.insert(name, skill);
    }

    pub async fn unregister(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.write().await.remove(name)
    }

    pub async fn get(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.read().await.get(name).cloned()
    }

    pub async fn list(&self) -> Vec<SkillMeta> {
        self.skills
            .read()
            .await
            .values()
            .map(|s| SkillMeta {
                name: s.name.clone(),
                description: s.description.clone(),
                triggers: s.triggers.iter().map(|t| t.trigger_type.clone()).collect(),
                tool_count: s.tools.len(),
            })
            .collect()
    }

    pub async fn find_by_trigger(&self, input: &str) -> Vec<Arc<Skill>> {
        self.skills
            .read()
            .await
            .values()
            .filter(|s| s.enabled && s.triggers.iter().any(|t| t.matches(input)))
            .cloned()
            .collect()
    }

    pub async fn find_by_type(&self, trigger_type: &SkillTriggerType) -> Vec<Arc<Skill>> {
        self.skills
            .read()
            .await
            .values()
            .filter(|s| s.enabled && s.triggers.iter().any(|t| &t.trigger_type == trigger_type))
            .cloned()
            .collect()
    }

    pub async fn load_from_paths(&self) -> usize {
        let metas = self.loader.list_skills();
        let mut count = 0;
        for meta in metas {
            if let Ok(Some(skill)) = self.loader.load_skill_by_name(&meta.name) {
                self.register(Arc::new(skill)).await;
                count += 1;
            }
        }
        info!("Loaded {} skills from search paths", count);
        count
    }

    pub async fn load_with_deps(&self) -> Result<usize, super::resolver::ResolverError> {
        let metas = self.loader.list_skills();
        let mut skills = Vec::new();

        for meta in metas {
            if let Ok(Some(skill)) = self.loader.load_skill_by_name(&meta.name) {
                skills.push(skill);
            }
        }

        let ordered = self.resolver.resolve_order(&skills)?;
        let count = ordered.len();
        for skill in ordered {
            self.register(skill).await;
        }

        info!("Loaded {} skills with dependency resolution", count);
        Ok(count)
    }

    pub async fn contains(&self, name: &str) -> bool {
        self.skills.read().await.contains_key(name)
    }

    pub async fn count(&self) -> usize {
        self.skills.read().await.len()
    }

    pub async fn clear(&self) {
        self.skills.write().await.clear();
    }

    pub async fn reload(&self) -> usize {
        self.clear().await;
        self.load_from_paths().await
    }

    pub fn prepare(&self, name: &str, context: &SkillContext) -> Option<PreparedSkill> {
        let skills = self.skills.blocking_read();
        skills.get(name).map(|skill| {
            let executor = super::executor::SkillExecutor::new();
            executor.prepare_execution(skill, context)
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Skill execution failed: {0}")]
    ExecutionFailed(String),
    #[error("No matching skill found")]
    NoMatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = SkillRegistry::new();
        registry
            .register(Arc::new(Skill {
                name: "test".to_string(),
                description: "Test skill".to_string(),
                triggers: vec![],
                content: "Test content".to_string(),
                tools: vec![],
                source_path: None,
                enabled: true,
                depends_on: vec![],
            }))
            .await;

        let retrieved = registry.get("test").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "Test skill");
    }

    #[tokio::test]
    async fn test_find_by_trigger() {
        let registry = SkillRegistry::new();
        registry
            .register(Arc::new(Skill {
                name: "review".to_string(),
                description: "Code review".to_string(),
                triggers: vec![
                    super::super::types::SkillTrigger::command("/review"),
                    super::super::types::SkillTrigger::keyword("review"),
                ],
                content: "Review code".to_string(),
                tools: vec!["Read".to_string()],
                source_path: None,
                enabled: true,
                depends_on: vec![],
            }))
            .await;

        let by_command = registry.find_by_trigger("/review").await;
        assert_eq!(by_command.len(), 1);
        assert_eq!(by_command[0].name, "review");
    }
}
