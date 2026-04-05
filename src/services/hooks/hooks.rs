//! Language-agnostic hook implementations
//!
//! Provides hooks that automatically adapt to the detected project type.

use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

use super::detector::{Formatter, Linter, ProjectInfo, TestFramework};
use super::traits::{HookCategory, HookContext, HookError, HookResult, Language, PreCommitHook};

/// Format check hook - automatically uses the correct formatter for the project
pub struct FormatHook {
    project_info: ProjectInfo,
}

impl FormatHook {
    pub fn new(project_info: ProjectInfo) -> Self {
        Self { project_info }
    }

    fn run_formatter(&self, formatter: Formatter, worktree: &Path) -> Result<HookResult, HookError> {
        let (cmd, args) = match formatter {
            Formatter::Rustfmt => ("cargo", vec!["fmt", "--check"]),
            Formatter::Prettier => ("npx", vec!["prettier", "--check", "."]),
            Formatter::Biome => ("npx", vec!["biome", "check", "--write", "."]),
            Formatter::Dprint => ("dprint", vec!["check"]),
            Formatter::Black => ("black", vec!["--check", "."]),
            Formatter::RuffFormat => ("ruff", vec!["format", "--check", "."]),
            Formatter::Autopep8 => ("autopep8", vec!["--check", "--diff", "-r", "."]),
            Formatter::Gofmt => ("gofmt", vec!["-l", "."]),
            Formatter::Goimports => ("goimports", vec!["-l", "."]),
            Formatter::GoogleJavaFormat => ("google-java-format", vec!["--dry-run", "--set-exit-if-changed", "**/*.java"]),
            Formatter::Rubocop => ("bundle", vec!["exec", "rubocop", "--format", "emacs"]),
            Formatter::PhpCsFixer => ("php-cs-fixer", vec!["fix", "--dry-run", "--diff"]),
        };

        debug!(formatter = formatter.as_str(), "Running format check");

        let output = Command::new(cmd)
            .args(&args)
            .current_dir(worktree)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HookError::ToolNotFound(cmd.to_string())
                } else {
                    HookError::Io(e)
                }
            })?;

        if output.status.success() {
            debug!(formatter = formatter.as_str(), "Format check passed");
            Ok(HookResult::Pass)
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut message = format!("Format check failed ({}):\n", formatter.as_str());

            if !stdout.is_empty() {
                message.push_str(&format!("{}\n", stdout));
            }
            if !stderr.is_empty() {
                message.push_str(&format!("{}\n", stderr));
            }

            warn!(formatter = formatter.as_str(), "Format check failed");
            Ok(HookResult::Fail(message))
        }
    }
}

impl PreCommitHook for FormatHook {
    fn id(&self) -> &str {
        "format"
    }

    fn name(&self) -> &str {
        "Format Check"
    }

    fn category(&self) -> HookCategory {
        HookCategory::Format
    }

    fn language(&self) -> Option<Language> {
        None // Language-agnostic
    }

    fn is_applicable(&self, _ctx: &HookContext) -> bool {
        self.project_info.formatter.is_some()
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if let Some(formatter) = &self.project_info.formatter {
            self.run_formatter(*formatter, &ctx.worktree_path)
        } else {
            Ok(HookResult::Skip("No formatter configured".to_string()))
        }
    }
}

/// Lint check hook - automatically uses the correct linter for the project
pub struct LintHook {
    project_info: ProjectInfo,
}

impl LintHook {
    pub fn new(project_info: ProjectInfo) -> Self {
        Self { project_info }
    }

    fn run_linter(&self, linter: Linter, worktree: &Path) -> Result<HookResult, HookError> {
        let (cmd, args) = match linter {
            Linter::Clippy => ("cargo", vec!["clippy", "--all-targets", "--", "-D", "warnings"]),
            Linter::Eslint => ("npx", vec!["eslint", ".", "--max-warnings", "0"]),
            Linter::BiomeLint => ("npx", vec!["biome", "lint", "."]),
            Linter::Ruff => ("ruff", vec!["check", "."]),
            Linter::Pylint => ("pylint", vec!["src", "tests"]),
            Linter::Flake8 => ("flake8", vec![".", "--max-line-length=100"]),
            Linter::Mypy => ("mypy", vec!["."]),
            Linter::GoVet => ("go", vec!["vet", "./..."]),
            Linter::Staticcheck => ("staticcheck", vec!["./..."]),
            Linter::Checkstyle => ("mvn", vec!["checkstyle:check"]),
            Linter::Rubocop => ("bundle", vec!["exec", "rubocop", "--format", "emacs"]),
            Linter::PhpCs => ("phpcs", vec!["--standard=PSR12", "."]),
        };

        debug!(linter = linter.as_str(), "Running lint check");

        let output = Command::new(cmd)
            .args(&args)
            .current_dir(worktree)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HookError::ToolNotFound(cmd.to_string())
                } else {
                    HookError::Io(e)
                }
            })?;

        if output.status.success() {
            debug!(linter = linter.as_str(), "Lint check passed");
            Ok(HookResult::Pass)
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut message = format!("Lint check failed ({}):\n", linter.as_str());

            if !stdout.is_empty() {
                message.push_str(&format!("{}\n", stdout));
            }
            if !stderr.is_empty() {
                message.push_str(&format!("{}\n", stderr));
            }

            warn!(linter = linter.as_str(), "Lint check failed");
            Ok(HookResult::Fail(message))
        }
    }
}

impl PreCommitHook for LintHook {
    fn id(&self) -> &str {
        "lint"
    }

    fn name(&self) -> &str {
        "Lint Check"
    }

    fn category(&self) -> HookCategory {
        HookCategory::Lint
    }

    fn language(&self) -> Option<Language> {
        None // Language-agnostic
    }

    fn is_applicable(&self, _ctx: &HookContext) -> bool {
        self.project_info.linter.is_some()
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if let Some(linter) = &self.project_info.linter {
            self.run_linter(*linter, &ctx.worktree_path)
        } else {
            Ok(HookResult::Skip("No linter configured".to_string()))
        }
    }
}

/// Test check hook - automatically uses the correct test framework for the project
pub struct TestHook {
    project_info: ProjectInfo,
}

impl TestHook {
    pub fn new(project_info: ProjectInfo) -> Self {
        Self { project_info }
    }

    fn run_tests(&self, framework: TestFramework, worktree: &Path) -> Result<HookResult, HookError> {
        let (cmd, args) = match framework {
            TestFramework::CargoTest => ("cargo", vec!["test", "--color=always"]),
            TestFramework::Jest => ("npx", vec!["jest", "--passWithNoTests"]),
            TestFramework::Vitest => ("npx", vec!["vitest", "run", "--passWithNoTests"]),
            TestFramework::Mocha => ("npx", vec!["mocha"]),
            TestFramework::Pytest => ("pytest", vec!["-v", "--tb=short"]),
            TestFramework::Unittest => ("python", vec!["-m", "unittest", "discover", "-v"]),
            TestFramework::GoTest => ("go", vec!["test", "-v", "./..."]),
            TestFramework::Junit => ("mvn", vec!["test"]),
            TestFramework::Rspec => ("bundle", vec!["exec", "rspec"]),
            TestFramework::Pest => ("vendor/bin/pest", vec![]),
            TestFramework::PhpUnit => ("vendor/bin/phpunit", vec![]),
        };

        debug!(framework = framework.as_str(), "Running tests");

        let output = Command::new(cmd)
            .args(&args)
            .current_dir(worktree)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HookError::ToolNotFound(cmd.to_string())
                } else {
                    HookError::Io(e)
                }
            })?;

        if output.status.success() {
            debug!(framework = framework.as_str(), "Tests passed");
            Ok(HookResult::Pass)
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut message = format!("Tests failed ({}):\n", framework.as_str());

            if !stdout.is_empty() {
                message.push_str(&format!("{}\n", stdout));
            }
            if !stderr.is_empty() {
                message.push_str(&format!("{}\n", stderr));
            }

            warn!(framework = framework.as_str(), "Tests failed");
            Ok(HookResult::Fail(message))
        }
    }
}

impl PreCommitHook for TestHook {
    fn id(&self) -> &str {
        "test"
    }

    fn name(&self) -> &str {
        "Test Check"
    }

    fn category(&self) -> HookCategory {
        HookCategory::Test
    }

    fn language(&self) -> Option<Language> {
        None // Language-agnostic
    }

    fn is_applicable(&self, _ctx: &HookContext) -> bool {
        self.project_info.test_framework.is_some()
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if let Some(framework) = &self.project_info.test_framework {
            self.run_tests(*framework, &ctx.worktree_path)
        } else {
            Ok(HookResult::Skip("No test framework configured".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context() -> HookContext {
        HookContext::default()
    }

    #[test]
    fn test_format_hook_id_name() {
        let project_info = ProjectInfo {
            languages: vec![],
            package_manager: None,
            formatter: None,
            linter: None,
            test_framework: None,
        };
        let hook = FormatHook::new(project_info);
        assert_eq!(hook.id(), "format");
        assert_eq!(hook.name(), "Format Check");
    }

    #[test]
    fn test_lint_hook_id_name() {
        let project_info = ProjectInfo {
            languages: vec![],
            package_manager: None,
            formatter: None,
            linter: None,
            test_framework: None,
        };
        let hook = LintHook::new(project_info);
        assert_eq!(hook.id(), "lint");
        assert_eq!(hook.name(), "Lint Check");
    }

    #[test]
    fn test_test_hook_id_name() {
        let project_info = ProjectInfo {
            languages: vec![],
            package_manager: None,
            formatter: None,
            linter: None,
            test_framework: None,
        };
        let hook = TestHook::new(project_info);
        assert_eq!(hook.id(), "test");
        assert_eq!(hook.name(), "Test Check");
    }

    #[test]
    fn test_hooks_not_applicable_without_tools() {
        let project_info = ProjectInfo {
            languages: vec![],
            package_manager: None,
            formatter: None,
            linter: None,
            test_framework: None,
        };

        let ctx = make_context();

        let format_hook = FormatHook::new(project_info.clone());
        assert!(!format_hook.is_applicable(&ctx));

        let lint_hook = LintHook::new(project_info.clone());
        assert!(!lint_hook.is_applicable(&ctx));

        let test_hook = TestHook::new(project_info);
        assert!(!test_hook.is_applicable(&ctx));
    }
}
