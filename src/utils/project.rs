//! Project Detection Utilities
//!
//! Ported from src/utils/detect-project.ts

use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct DetectedProject {
    pub language: String,
    pub framework: String,
    pub package_manager: String,
    pub entry_point: String,
    pub build_command: String,
    pub test_command: String,
    pub description: String,
}

pub fn detect_project(project_dir: &Path) -> DetectedProject {
    // Current directory detectors

    // Node.js (package.json)
    let package_json_path = project_dir.join("package.json");
    if package_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                let mut detected = DetectedProject::default();
                detected.language = "JavaScript".to_string();
                detected.framework = "Node.js".to_string();
                detected.package_manager = "npm".to_string();

                let deps = pkg.get("dependencies").and_then(|d| d.as_object());
                let dev_deps = pkg.get("devDependencies").and_then(|d| d.as_object());

                let has_dep = |name: &str| {
                    deps.map_or(false, |d| d.contains_key(name))
                        || dev_deps.map_or(false, |d| d.contains_key(name))
                };

                if has_dep("next") {
                    detected.framework = "Next.js".to_string();
                } else if has_dep("vite") {
                    detected.framework = "Vite".to_string();
                } else if has_dep("react") {
                    detected.framework = "React".to_string();
                } else if has_dep("express") {
                    detected.framework = "Express".to_string();
                }

                if has_dep("typescript") || project_dir.join("tsconfig.json").exists() {
                    detected.language = "TypeScript".to_string();
                }

                if project_dir.join("bun.lock").exists() || project_dir.join("bunfig.toml").exists()
                {
                    detected.package_manager = "bun".to_string();
                } else if project_dir.join("pnpm-lock.yaml").exists() {
                    detected.package_manager = "pnpm".to_string();
                } else if project_dir.join("yarn.lock").exists() {
                    detected.package_manager = "yarn".to_string();
                }

                detected.entry_point = pkg
                    .get("main")
                    .or(pkg.get("module"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("src/index.ts")
                    .to_string();

                let pm = &detected.package_manager;
                if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
                    if scripts.contains_key("build") {
                        detected.build_command = format!("{} run build", pm);
                    }
                    if scripts.contains_key("test") {
                        detected.test_command = format!("{} test", pm);
                    }
                }

                detected.description = pkg
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                return detected;
            }
        }
    }

    // Rust (Cargo.toml)
    let cargo_toml_path = project_dir.join("Cargo.toml");
    if cargo_toml_path.exists() {
        if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
            let mut detected = DetectedProject::default();
            detected.language = "Rust".to_string();
            detected.framework = "Cargo".to_string();
            detected.package_manager = "cargo".to_string();
            detected.entry_point = "src/main.rs".to_string();
            detected.build_command = "cargo build".to_string();
            detected.test_command = "cargo test".to_string();

            if let Some(line) = content.lines().find(|l| l.trim().starts_with("name")) {
                if let Some(name) = line.split('"').nth(1) {
                    detected.description = format!("Rust project: {}", name);
                }
            }
            return detected;
        }
    }

    // Default
    DetectedProject {
        language: "Unknown".to_string(),
        framework: "Unknown".to_string(),
        package_manager: "Unknown".to_string(),
        ..Default::default()
    }
}

pub fn generate_project_md(detected: &DetectedProject) -> String {
    let mut lines = vec![
        "# Project Overview".to_string(),
        "".to_string(),
        format!("**Language:** {}", detected.language),
        format!("**Framework:** {}", detected.framework),
        format!("**Package Manager:** {}", detected.package_manager),
    ];

    if !detected.entry_point.is_empty() {
        lines.push(format!("**Entry Point:** {}", detected.entry_point));
    }
    if !detected.build_command.is_empty() {
        lines.push(format!("**Build:** `{}`", detected.build_command));
    }
    if !detected.test_command.is_empty() {
        lines.push(format!("**Test:** `{}`", detected.test_command));
    }
    if !detected.description.is_empty() {
        lines.push("".to_string());
        lines.push(format!("> {}", detected.description));
    }

    lines.push("".to_string());
    lines.push("## Conventions".to_string());
    lines.push("".to_string());
    lines.push("<!-- Add your project conventions here -->".to_string());
    lines.push("<!-- The agent will follow these when writing code -->".to_string());
    lines.push("".to_string());
    lines.push("## Important Files".to_string());
    lines.push("".to_string());
    lines.push("<!-- List key files the agent should know about -->".to_string());
    lines.push("".to_string());

    lines.join("\n")
}
