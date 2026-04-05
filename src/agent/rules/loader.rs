//! Loading logic for per-project agent rules.

use std::path::Path;
use tracing::{debug, warn};

use super::types::{ProjectRules, RulesFile};

impl ProjectRules {
    /// Load rules from a project root directory.
    /// Tries sources in order:
    /// 1. `.d3vx/rules.yaml` / `.d3vx/rules.yml` (primary)
    /// 2. `.d3vx/project.md` (description fallback)
    /// 3. `docs/ARCHITECTURE.md` (supplementary doc)
    pub fn load(project_root: &Path) -> Self {
        let mut rules = Self::default();

        // 1. Try .d3vx/rules.yaml
        let rules_yaml = project_root.join(".d3vx/rules.yaml");
        if rules_yaml.exists() {
            if let Some(loaded) = Self::load_rules_yaml(&rules_yaml) {
                debug!("Loaded rules from {}", rules_yaml.display());
                rules = loaded;
            }
        } else {
            // 2. Try .d3vx/rules.yml
            let rules_yml = project_root.join(".d3vx/rules.yml");
            if rules_yml.exists() {
                if let Some(loaded) = Self::load_rules_yaml(&rules_yml) {
                    debug!("Loaded rules from {}", rules_yml.display());
                    rules = loaded;
                }
            }
        }

        // 3. Load .d3vx/project.md as description fallback
        let project_md = project_root.join(".d3vx/project.md");
        if project_md.exists() && rules.description.is_none() {
            if let Ok(content) = std::fs::read_to_string(&project_md) {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    rules.description = Some(trimmed);
                    debug!("Loaded project description from project.md");
                }
            }
        }

        // 4. Load docs/ARCHITECTURE.md
        if let Some(arch_doc) = Self::load_architecture_md(project_root) {
            rules.architecture_doc = Some(arch_doc);
        }

        rules
    }

    /// Parse a YAML rules file into `ProjectRules`.
    fn load_rules_yaml(rules_path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(rules_path)
            .map_err(|e| {
                warn!("Failed to read rules file {}: {}", rules_path.display(), e);
                e
            })
            .ok()?;
        let parsed: RulesFile = serde_yaml::from_str(&content)
            .map_err(|e| {
                warn!(
                    "Failed to parse rules YAML at {}: {}",
                    rules_path.display(),
                    e
                );
                e
            })
            .ok()?;
        Some(Self {
            description: parsed.description,
            constraints: parsed.constraints.unwrap_or_default(),
            conventions: parsed.conventions.unwrap_or_default(),
            protected_paths: parsed.protected_paths.unwrap_or_default(),
            system_prompt_additions: parsed.system_prompt.unwrap_or_default(),
            role_rules: parsed.roles.unwrap_or_default(),
            architecture_doc: None,
        })
    }

    /// Load the contents of `docs/ARCHITECTURE.md` if present.
    fn load_architecture_md(project_root: &Path) -> Option<String> {
        let arch_path = project_root.join("docs/ARCHITECTURE.md");
        if !arch_path.exists() {
            return None;
        }
        std::fs::read_to_string(&arch_path)
            .map_err(|e| {
                warn!("Failed to read docs/ARCHITECTURE.md: {}", e);
                e
            })
            .ok()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .inspect(|_| debug!("Loaded architecture doc from docs/ARCHITECTURE.md"))
    }
}
