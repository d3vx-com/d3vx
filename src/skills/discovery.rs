//! Skill Auto-Discovery
//!
//! Automatically discovers and loads skills from standard locations:
//! - `~/.d3vx/skills/` — global skills
//! - `.d3vx/skills/` — project-local skills
//!
//! Also provides skill import command for loading external skills.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{info, warn};

use super::loader::SkillLoader;
use super::registry::SkillRegistry;
use super::resolver::SkillResolver;

/// Result of a discovery operation.
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    pub skills_found: usize,
    pub skills_loaded: usize,
    pub errors: Vec<String>,
    pub source_paths: Vec<String>,
}

impl DiscoveryResult {
    pub fn is_empty(&self) -> bool {
        self.skills_found == 0 && self.source_paths.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.skills_found == 0 {
            "No skills found".to_string()
        } else {
            format!(
                "Found {} skills, loaded {} into registry{}",
                self.skills_found,
                self.skills_loaded,
                if self.errors.is_empty() {
                    String::new()
                } else {
                    format!(" ({} errors)", self.errors.len())
                },
            )
        }
    }
}

/// Discovers skills from standard locations.
pub struct SkillDiscovery {
    project_root: Option<PathBuf>,
}

impl SkillDiscovery {
    pub fn new() -> Self {
        Self { project_root: None }
    }

    pub fn with_project_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.project_root = Some(path.into());
        self
    }

    /// Discover and load skills from all standard locations.
    pub async fn discover_all(&self, registry: &SkillRegistry) -> DiscoveryResult {
        let paths = self.collect_search_paths();

        if paths.is_empty() {
            return DiscoveryResult {
                skills_found: 0,
                skills_loaded: 0,
                errors: vec!["No search paths configured".to_string()],
                source_paths: vec![],
            };
        }

        // Build loader with all paths (last path has highest priority)
        let mut loader = SkillLoader::new();
        for path in &paths {
            loader = loader.add_search_path(path.clone());
        }

        // List available skills
        let metas = loader.list_skills();
        let source_paths: Vec<_> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        info!(
            count = metas.len(),
            paths = source_paths.join(", "),
            "Discovered skills from standard paths"
        );

        // Load all skills
        let mut errors = Vec::new();
        let skills: Vec<_> = metas
            .iter()
            .filter_map(|meta| match loader.load_skill_by_name(&meta.name) {
                Ok(Some(skill)) => Some(skill),
                Ok(None) => {
                    errors.push(format!("Could not load skill: {}", meta.name));
                    None
                }
                Err(e) => {
                    errors.push(format!("Failed to load {}: {}", meta.name, e));
                    None
                }
            })
            .collect();

        // Resolve dependencies
        let resolver = SkillResolver::new();
        let ordered = match resolver.resolve_order(&skills) {
            Ok(ordered) => ordered,
            Err(e) => {
                warn!("Dependency resolution failed: {e}, loading in discovery order");
                skills.into_iter().map(|s| Arc::new(s)).collect::<Vec<_>>()
            }
        };

        let count = ordered.len();
        for skill in ordered {
            registry.register(skill).await;
        }

        DiscoveryResult {
            skills_found: metas.len(),
            skills_loaded: count,
            errors,
            source_paths,
        }
    }

    /// Collect search paths in priority order (global → local).
    fn collect_search_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Global skills (~/.d3vx/skills/)
        if let Some(home) = dirs::home_dir() {
            let global = home.join(".d3vx").join("skills");
            if global.exists() {
                paths.push(global);
            }
        }

        // 2. Project-local skills (.d3vx/skills/)
        if let Some(root) = &self.project_root {
            let local = root.join(".d3vx").join("skills");
            if local.exists() {
                paths.push(local);
            }
        }

        paths
    }

    /// Import a skill from a file or directory.
    ///
    /// Destination is `.d3vx/skills/<name>/`.
    pub async fn import_skill(
        &self,
        source: &Path,
        registry: &SkillRegistry,
    ) -> std::io::Result<ImportResult> {
        let target_base = self
            .project_root
            .as_ref()
            .map(|p| p.join(".d3vx"))
            .or_else(|| dirs::home_dir().map(|h| h.join(".d3vx")))
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Neither project root nor home directory available",
                )
            })?
            .join("skills");

        std::fs::create_dir_all(&target_base)?;

        let is_dir = source.is_dir();
        let (name, dest) = if is_dir {
            let name = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let dest = target_base.join(&name);
            std::fs::create_dir_all(&dest)?;
            if source.join("SKILL.md").exists() {
                std::fs::copy(source.join("SKILL.md"), dest.join("SKILL.md"))?;
            }
            (name, dest)
        } else {
            let stem = source
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let dest = target_base.join(&stem);
            std::fs::create_dir_all(&dest)?;
            std::fs::copy(source, dest.join("SKILL.md"))?;
            (stem, dest)
        };

        let loader = SkillLoader::new().add_search_path(dest.clone());
        let loaded = loader.load_skill(&dest.join("SKILL.md")).ok();

        if let Some(skill) = loaded {
            registry.register(Arc::new(skill)).await;
            Ok(ImportResult {
                name,
                path: dest,
                success: true,
            })
        } else {
            warn!("Imported skill but failed to load from {dest:?}");
            Ok(ImportResult {
                name,
                path: dest,
                success: false,
            })
        }
    }
}

/// Result of importing a skill.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub name: String,
    pub path: PathBuf,
    pub success: bool,
}

impl std::fmt::Display for ImportResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.success {
            "OK"
        } else {
            "Imported (load failed)"
        };
        write!(f, "{}: {} ({})", self.name, self.path.display(), status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_discovery() {
        let discovery = SkillDiscovery::new();
        let registry = super::super::registry::SkillRegistry::new();
        let result = discovery.discover_all(&registry).await;
        assert!(result.skills_found == 0 || result.source_paths.is_empty());
    }

    #[test]
    fn test_import_result_display() {
        let r = ImportResult {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test"),
            success: true,
        };
        let text = format!("{r}");
        assert!(text.contains("test"));
        assert!(text.contains("OK"));
    }
}
