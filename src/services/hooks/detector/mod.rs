//! Project type detection
//!
//! Automatically detects the programming languages and tools used in a project.

mod types;

pub use types::*;

use std::collections::HashSet;
use std::path::Path;
use tracing::info;

use super::traits::Language;

/// Project detector for auto-discovering project configuration
pub struct ProjectDetector {
    /// Root path to detect from
    root: std::path::PathBuf,
}

impl ProjectDetector {
    /// Create a new detector for the given path
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Detect all project information
    pub fn detect(&self) -> ProjectInfo {
        let languages = self.detect_languages();
        let package_manager = self.detect_package_manager(&languages);
        let formatter = self.detect_formatter(&languages);
        let linter = self.detect_linter(&languages);
        let test_framework = self.detect_test_framework(&languages);

        let info = ProjectInfo {
            languages,
            package_manager,
            formatter,
            linter,
            test_framework,
        };

        info!(?info, "Detected project information");
        info
    }

    /// Detect programming languages used in the project
    pub fn detect_languages(&self) -> Vec<Language> {
        let mut languages = HashSet::new();

        if self.has_file("Cargo.toml") {
            languages.insert(Language::Rust);
        }

        if self.has_file("package.json") {
            if self.has_file("tsconfig.json") || self.has_any_extension(&["ts", "tsx"]) {
                languages.insert(Language::TypeScript);
            }
            languages.insert(Language::JavaScript);
        }

        if self.has_any_file(&["pyproject.toml", "setup.py", "requirements.txt", "Pipfile"]) {
            languages.insert(Language::Python);
        } else if self.has_any_extension(&["py"]) {
            languages.insert(Language::Python);
        }

        if self.has_file("go.mod") {
            languages.insert(Language::Go);
        }

        if self.has_any_file(&["pom.xml", "build.gradle", "build.gradle.kts"]) {
            languages.insert(Language::Java);
        }

        if self.has_file("Gemfile") {
            languages.insert(Language::Ruby);
        }

        if self.has_file("composer.json") {
            languages.insert(Language::Php);
        }

        let mut result: Vec<_> = languages.into_iter().collect();
        result.sort_by_key(|l| l.display_name().to_string());
        result
    }

    /// Detect the package manager
    pub fn detect_package_manager(&self, languages: &[Language]) -> Option<PackageManager> {
        if languages.contains(&Language::Rust) {
            return Some(PackageManager::Cargo);
        }

        if languages.contains(&Language::JavaScript) || languages.contains(&Language::TypeScript) {
            if self.has_file("pnpm-lock.yaml") {
                return Some(PackageManager::Pnpm);
            }
            if self.has_file("yarn.lock") {
                return Some(PackageManager::Yarn);
            }
            if self.has_file("bun.lockb") {
                return Some(PackageManager::Bun);
            }
            return Some(PackageManager::Npm);
        }

        if languages.contains(&Language::Python) {
            if self.has_file("uv.lock") {
                return Some(PackageManager::Uv);
            }
            if self.has_file("poetry.lock") {
                return Some(PackageManager::Poetry);
            }
            if self.has_file("Pipfile.lock") {
                return Some(PackageManager::Pipenv);
            }
            return Some(PackageManager::Pip);
        }

        if languages.contains(&Language::Go) {
            return Some(PackageManager::GoModules);
        }

        if languages.contains(&Language::Ruby) {
            return Some(PackageManager::Bundler);
        }

        if languages.contains(&Language::Php) {
            return Some(PackageManager::Composer);
        }

        None
    }

    /// Detect the formatter
    pub fn detect_formatter(&self, languages: &[Language]) -> Option<Formatter> {
        if languages.contains(&Language::Rust) {
            return Some(Formatter::Rustfmt);
        }

        if languages.contains(&Language::JavaScript) || languages.contains(&Language::TypeScript) {
            if self.has_file("biome.json") || self.file_contains("package.json", "biome") {
                return Some(Formatter::Biome);
            }
            if self.has_file("dprint.json") {
                return Some(Formatter::Dprint);
            }
            return Some(Formatter::Prettier);
        }

        if languages.contains(&Language::Python) {
            if self.file_contains("pyproject.toml", "ruff") {
                return Some(Formatter::RuffFormat);
            }
            return Some(Formatter::Black);
        }

        if languages.contains(&Language::Go) {
            return Some(Formatter::Gofmt);
        }

        if languages.contains(&Language::Java) {
            return Some(Formatter::GoogleJavaFormat);
        }

        if languages.contains(&Language::Ruby) {
            return Some(Formatter::Rubocop);
        }

        if languages.contains(&Language::Php) {
            return Some(Formatter::PhpCsFixer);
        }

        None
    }

    /// Detect the linter
    pub fn detect_linter(&self, languages: &[Language]) -> Option<Linter> {
        if languages.contains(&Language::Rust) {
            return Some(Linter::Clippy);
        }

        if languages.contains(&Language::JavaScript) || languages.contains(&Language::TypeScript) {
            if self.has_file("biome.json") || self.file_contains("package.json", "biome") {
                return Some(Linter::BiomeLint);
            }
            return Some(Linter::Eslint);
        }

        if languages.contains(&Language::Python) {
            if self.file_contains("pyproject.toml", "ruff") {
                return Some(Linter::Ruff);
            }
            if self.file_contains("pyproject.toml", "mypy") {
                return Some(Linter::Mypy);
            }
            return Some(Linter::Ruff);
        }

        if languages.contains(&Language::Go) {
            return Some(Linter::GoVet);
        }

        if languages.contains(&Language::Java) {
            return Some(Linter::Checkstyle);
        }

        if languages.contains(&Language::Ruby) {
            return Some(Linter::Rubocop);
        }

        if languages.contains(&Language::Php) {
            return Some(Linter::PhpCs);
        }

        None
    }

    /// Detect the test framework
    pub fn detect_test_framework(&self, languages: &[Language]) -> Option<TestFramework> {
        if languages.contains(&Language::Rust) {
            return Some(TestFramework::CargoTest);
        }

        if languages.contains(&Language::JavaScript) || languages.contains(&Language::TypeScript) {
            if self.file_contains("package.json", "vitest") {
                return Some(TestFramework::Vitest);
            }
            if self.file_contains("package.json", "jest") {
                return Some(TestFramework::Jest);
            }
            if self.file_contains("package.json", "mocha") {
                return Some(TestFramework::Mocha);
            }
            return Some(TestFramework::Jest);
        }

        if languages.contains(&Language::Python) {
            return Some(TestFramework::Pytest);
        }

        if languages.contains(&Language::Go) {
            return Some(TestFramework::GoTest);
        }

        if languages.contains(&Language::Java) {
            return Some(TestFramework::Junit);
        }

        if languages.contains(&Language::Ruby) {
            return Some(TestFramework::Rspec);
        }

        if languages.contains(&Language::Php) {
            return Some(TestFramework::Pest);
        }

        None
    }

    // Helper methods

    fn has_file(&self, name: &str) -> bool {
        self.root.join(name).exists()
    }

    fn has_any_file(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.has_file(name))
    }

    fn has_any_extension(&self, extensions: &[&str]) -> bool {
        use std::fs;
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_file() {
                        if let Some(ext) = entry.path().extension() {
                            if extensions.iter().any(|e| ext == *e) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn file_contains(&self, filename: &str, pattern: &str) -> bool {
        let path = self.root.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            return content.to_lowercase().contains(&pattern.to_lowercase());
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let detector = ProjectDetector::new(temp_dir.path());
        let info = detector.detect();

        assert!(info.languages.contains(&Language::Rust));
        assert_eq!(info.package_manager, Some(PackageManager::Cargo));
        assert_eq!(info.formatter, Some(Formatter::Rustfmt));
        assert_eq!(info.linter, Some(Linter::Clippy));
        assert_eq!(info.test_framework, Some(TestFramework::CargoTest));
    }

    #[test]
    fn test_detect_typescript_project() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"name": "test", "devDependencies": {"typescript": "^5.0.0", "vitest": "^1.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(temp_dir.path().join("tsconfig.json"), "{}").unwrap();

        let detector = ProjectDetector::new(temp_dir.path());
        let info = detector.detect();

        assert!(info.languages.contains(&Language::TypeScript));
        assert!(info.languages.contains(&Language::JavaScript));
        assert_eq!(info.test_framework, Some(TestFramework::Vitest));
    }

    #[test]
    fn test_detect_python_project() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("pyproject.toml"),
            "[tool.ruff]\n[tool.pytest]",
        )
        .unwrap();

        let detector = ProjectDetector::new(temp_dir.path());
        let info = detector.detect();

        assert!(info.languages.contains(&Language::Python));
        assert_eq!(info.formatter, Some(Formatter::RuffFormat));
        assert_eq!(info.linter, Some(Linter::Ruff));
    }

    #[test]
    fn test_detect_go_project() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("go.mod"), "module example.com/test\n").unwrap();

        let detector = ProjectDetector::new(temp_dir.path());
        let info = detector.detect();

        assert!(info.languages.contains(&Language::Go));
        assert_eq!(info.package_manager, Some(PackageManager::GoModules));
        assert_eq!(info.formatter, Some(Formatter::Gofmt));
        assert_eq!(info.linter, Some(Linter::GoVet));
        assert_eq!(info.test_framework, Some(TestFramework::GoTest));
    }

    #[test]
    fn test_detect_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let detector = ProjectDetector::new(temp_dir.path());
        let info = detector.detect();

        assert!(info.languages.is_empty());
        assert!(info.package_manager.is_none());
    }
}
