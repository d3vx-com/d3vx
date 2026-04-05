//! Tests for per-project agent rules.

use super::types::*;
use std::collections::HashMap;
use std::io::Write as IoWrite;
/// Helper to create a temp directory with specific files.
fn temp_project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    for (path, content) in files {
        let full_path = dir.path().join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        let mut f = std::fs::File::create(&full_path).expect("create file");
        f.write_all(content.as_bytes()).expect("write file");
    }
    dir
}

#[test]
fn load_from_rules_yaml() {
    let dir = temp_project(&[(
        ".d3vx/rules.yaml",
        r#"
description: "Test project"
constraints:
  - "Never modify vendor/"
  - "All endpoints versioned"
conventions:
  - "Use snake_case for Rust"
protected_paths:
  - "vendor/**"
  - "*.lock"
system_prompt:
  - "Always run tests before committing"
roles:
  backend:
    - "Use Result<T, E>"
    - "No unwrap in production"
  frontend:
    - "Use TypeScript strict mode"
"#,
    )]);

    let rules = ProjectRules::load(dir.path());
    assert_eq!(rules.description.as_deref(), Some("Test project"));
    assert_eq!(rules.constraints.len(), 2);
    assert!(rules.constraints[0].contains("vendor"));
    assert_eq!(rules.conventions.len(), 1);
    assert_eq!(rules.protected_paths.len(), 2);
    assert_eq!(rules.system_prompt_additions.len(), 1);
    assert_eq!(rules.role_rules.get("backend").map(|r| r.len()), Some(2));
    assert_eq!(rules.role_rules.get("frontend").map(|r| r.len()), Some(1));
    assert!(rules.architecture_doc.is_none());
}

#[test]
fn load_from_rules_yml_extension() {
    let dir = temp_project(&[(
        ".d3vx/rules.yml",
        r#"
description: "YML variant"
constraints:
  - "Keep it simple"
"#,
    )]);

    let rules = ProjectRules::load(dir.path());
    assert_eq!(rules.description.as_deref(), Some("YML variant"));
    assert_eq!(rules.constraints.len(), 1);
}

#[test]
fn yaml_takes_precedence_over_yml() {
    let dir = temp_project(&[
        (".d3vx/rules.yaml", "description: \"yaml wins\""),
        (".d3vx/rules.yml", "description: \"yml loses\""),
    ]);

    let rules = ProjectRules::load(dir.path());
    assert_eq!(rules.description.as_deref(), Some("yaml wins"));
}

#[test]
fn load_project_md_as_fallback() {
    let dir = temp_project(&[(".d3vx/project.md", "# My Project\n\nA great project.")]);

    let rules = ProjectRules::load(dir.path());
    assert!(rules.description.is_some());
    assert!(rules.description.as_ref().unwrap().contains("My Project"));
}

#[test]
fn project_md_does_not_override_yaml_description() {
    let dir = temp_project(&[
        (".d3vx/rules.yaml", "description: \"from yaml\""),
        (".d3vx/project.md", "# Should be ignored"),
    ]);

    let rules = ProjectRules::load(dir.path());
    assert_eq!(rules.description.as_deref(), Some("from yaml"));
}

#[test]
fn load_architecture_md() {
    let dir = temp_project(&[(
        "docs/ARCHITECTURE.md",
        "# Architecture\n\nWe use a layered architecture.",
    )]);

    let rules = ProjectRules::load(dir.path());
    assert!(rules.architecture_doc.is_some());
    assert!(rules
        .architecture_doc
        .as_ref()
        .unwrap()
        .contains("layered architecture"));
}

#[test]
fn empty_project_returns_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let rules = ProjectRules::load(dir.path());
    assert!(rules.description.is_none());
    assert!(rules.constraints.is_empty());
    assert!(rules.conventions.is_empty());
    assert!(rules.architecture_doc.is_none());
    assert!(!rules.has_rules());
}

#[test]
fn to_prompt_section_formatting() {
    let dir = temp_project(&[
        (
            ".d3vx/rules.yaml",
            r#"
constraints:
  - "Never modify vendor/"
conventions:
  - "Use snake_case"
roles:
  backend:
    - "Use Result<T, E>"
"#,
        ),
        ("docs/ARCHITECTURE.md", "Layered architecture."),
    ]);

    let rules = ProjectRules::load(dir.path());
    let prompt = rules.to_prompt_section();

    assert!(prompt.starts_with("# Project Rules"));
    assert!(prompt.contains("## Constraints"));
    assert!(prompt.contains("Never modify vendor/"));
    assert!(prompt.contains("## Conventions"));
    assert!(prompt.contains("Use snake_case"));
    assert!(prompt.contains("## Architecture Notes"));
    assert!(prompt.contains("Layered architecture."));
}

#[test]
fn role_prompt_section_returns_rules() {
    let dir = temp_project(&[(
        ".d3vx/rules.yaml",
        r#"
roles:
  backend:
    - "Use Result<T, E>"
    - "No unwrap"
"#,
    )]);

    let rules = ProjectRules::load(dir.path());
    let section = rules.role_prompt_section("backend").expect("should exist");
    assert!(section.contains("Role-specific: backend"));
    assert!(section.contains("Use Result<T, E>"));
    assert!(section.contains("No unwrap"));
}

#[test]
fn role_prompt_section_returns_none_for_unknown_role() {
    let rules = ProjectRules::default();
    assert!(rules.role_prompt_section("nonexistent").is_none());
}

#[test]
fn rules_for_role_is_case_insensitive() {
    let dir = temp_project(&[(
        ".d3vx/rules.yaml",
        r#"
roles:
  backend:
    - "Rule one"
"#,
    )]);

    let rules = ProjectRules::load(dir.path());
    assert_eq!(rules.rules_for_role("Backend").len(), 1);
    assert_eq!(rules.rules_for_role("BACKEND").len(), 1);
    assert_eq!(rules.rules_for_role("backend").len(), 1);
}

#[test]
fn is_protected_with_glob_patterns() {
    let dir = temp_project(&[(
        ".d3vx/rules.yaml",
        r#"
protected_paths:
  - "vendor/**"
  - "*.lock"
  - "src/generated/*"
"#,
    )]);

    let rules = ProjectRules::load(dir.path());
    assert!(rules.is_protected("vendor/dep.rs"));
    assert!(rules.is_protected("Cargo.lock"));
    assert!(rules.is_protected("src/generated/types.rs"));
    assert!(!rules.is_protected("src/main.rs"));
    assert!(!rules.is_protected("README.md"));
}

#[test]
fn is_protected_fallback_for_invalid_glob() {
    let rules = ProjectRules {
        protected_paths: vec!["[invalid".to_string()],
        ..Default::default()
    };
    // Invalid glob pattern falls back to contains match
    assert!(rules.is_protected("some/[invalid/path"));
}

#[test]
fn merge_combines_rules() {
    let base = ProjectRules {
        description: Some("Base".to_string()),
        constraints: vec!["C1".to_string()],
        conventions: vec!["V1".to_string()],
        protected_paths: vec!["P1".to_string()],
        system_prompt_additions: vec!["S1".to_string()],
        role_rules: {
            let mut m = HashMap::new();
            m.insert("backend".to_string(), vec!["R1".to_string()]);
            m
        },
        architecture_doc: Some("Base arch".to_string()),
    };

    let overlay = ProjectRules {
        description: Some("Overlay".to_string()),
        constraints: vec!["C2".to_string()],
        conventions: vec!["V1".to_string()], // duplicate, should not repeat
        protected_paths: vec!["P2".to_string()],
        system_prompt_additions: vec![],
        role_rules: {
            let mut m = HashMap::new();
            m.insert("backend".to_string(), vec!["R2".to_string()]);
            m.insert("frontend".to_string(), vec!["F1".to_string()]);
            m
        },
        architecture_doc: None,
    };

    let merged = ProjectRules::merge(base, overlay);
    assert_eq!(merged.description.as_deref(), Some("Overlay"));
    assert_eq!(merged.constraints, vec!["C1", "C2"]);
    assert_eq!(merged.conventions, vec!["V1"]); // no duplicate
    assert_eq!(merged.protected_paths, vec!["P1", "P2"]);
    assert_eq!(merged.system_prompt_additions, vec!["S1"]);
    assert_eq!(merged.role_rules.get("backend").unwrap().len(), 2);
    assert_eq!(merged.role_rules.get("frontend").unwrap().len(), 1);
    assert_eq!(merged.architecture_doc.as_deref(), Some("Base arch"));
}

#[test]
fn has_rules_returns_true_when_populated() {
    let rules = ProjectRules {
        constraints: vec!["One rule".to_string()],
        ..Default::default()
    };
    assert!(rules.has_rules());
}

#[test]
fn invalid_yaml_returns_none_gracefully() {
    let dir = temp_project(&[(".d3vx/rules.yaml", "this is not: valid: yaml: [[[")]);

    let rules = ProjectRules::load(dir.path());
    // Should not panic; returns defaults
    assert!(rules.description.is_none());
    assert!(rules.constraints.is_empty());
}

#[test]
fn empty_rules_file_returns_defaults() {
    let dir = temp_project(&[(".d3vx/rules.yaml", "")]);
    let rules = ProjectRules::load(dir.path());
    assert!(rules.description.is_none());
    assert!(rules.constraints.is_empty());
}
