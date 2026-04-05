//! Project detection type definitions

use super::traits::Language;

/// Detected project information
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    /// Detected languages
    pub languages: Vec<Language>,
    /// Package manager (if detected)
    pub package_manager: Option<PackageManager>,
    /// Formatter tool (if configured)
    pub formatter: Option<Formatter>,
    /// Linter tool (if configured)
    pub linter: Option<Linter>,
    /// Test framework (if configured)
    pub test_framework: Option<TestFramework>,
}

/// Package managers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Cargo,
    Npm,
    Yarn,
    Pnpm,
    Bun,
    Pip,
    Poetry,
    Pipenv,
    Uv,
    GoModules,
    Bundler,
    Composer,
}

/// Formatter tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formatter {
    Rustfmt,
    Prettier,
    Biome,
    Dprint,
    Black,
    RuffFormat,
    Autopep8,
    Gofmt,
    Goimports,
    GoogleJavaFormat,
    Rubocop,
    PhpCsFixer,
}

impl Formatter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Formatter::Rustfmt => "rustfmt",
            Formatter::Prettier => "prettier",
            Formatter::Biome => "biome",
            Formatter::Dprint => "dprint",
            Formatter::Black => "black",
            Formatter::RuffFormat => "ruff format",
            Formatter::Autopep8 => "autopep8",
            Formatter::Gofmt => "gofmt",
            Formatter::Goimports => "goimports",
            Formatter::GoogleJavaFormat => "google-java-format",
            Formatter::Rubocop => "rubocop",
            Formatter::PhpCsFixer => "php-cs-fixer",
        }
    }
}

/// Linter tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linter {
    Clippy,
    Eslint,
    BiomeLint,
    Ruff,
    Pylint,
    Flake8,
    Mypy,
    GoVet,
    Staticcheck,
    Checkstyle,
    Rubocop,
    PhpCs,
}

impl Linter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Linter::Clippy => "clippy",
            Linter::Eslint => "eslint",
            Linter::BiomeLint => "biome lint",
            Linter::Ruff => "ruff",
            Linter::Pylint => "pylint",
            Linter::Flake8 => "flake8",
            Linter::Mypy => "mypy",
            Linter::GoVet => "go vet",
            Linter::Staticcheck => "staticcheck",
            Linter::Checkstyle => "checkstyle",
            Linter::Rubocop => "rubocop",
            Linter::PhpCs => "phpcs",
        }
    }
}

/// Test frameworks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFramework {
    CargoTest,
    Jest,
    Vitest,
    Mocha,
    Pytest,
    Unittest,
    GoTest,
    Junit,
    Rspec,
    Pest,
    PhpUnit,
}

impl TestFramework {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestFramework::CargoTest => "cargo test",
            TestFramework::Jest => "jest",
            TestFramework::Vitest => "vitest",
            TestFramework::Mocha => "mocha",
            TestFramework::Pytest => "pytest",
            TestFramework::Unittest => "unittest",
            TestFramework::GoTest => "go test",
            TestFramework::Junit => "junit",
            TestFramework::Rspec => "rspec",
            TestFramework::Pest => "pest",
            TestFramework::PhpUnit => "phpunit",
        }
    }
}
