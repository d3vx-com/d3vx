//! Grading rules and adjudication.
//!
//! A grader inspects the post-task workspace and produces a
//! pass/fail verdict. Graders are declared in TOML and carry no behaviour
//! of their own beyond the logic defined here — they cannot execute
//! arbitrary Rust code, only shell commands and filesystem checks.
//!
//! The set of grader types is intentionally small:
//!
//! | Variant         | Passes when                                          |
//! |-----------------|------------------------------------------------------|
//! | `ShellCommand`  | Command exits with status 0 (configurable)           |
//! | `FileExists`    | Path exists in the workspace                         |
//! | `FileContains`  | File contains a substring and/or matches a regex     |
//! | `All`           | Every child grader passes                            |
//! | `Any`           | At least one child grader passes                     |
//!
//! New rule types can be added by extending the enum — the exhaustive
//! match in [`GraderSpec::grade`] forces every call site to handle them.

use std::fs;
use std::process::Command;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::environment::EvalEnvironment;

/// A grading rule. Serialised with an internally-tagged `type` field so
/// TOML can discriminate between variants:
///
/// ```toml
/// [[graders]]
/// type = "shell_command"
/// command = "cargo test --quiet"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraderSpec {
    /// Run a shell command in the workspace. Passes on exit status 0 by
    /// default; flip [`ShellCommand::pass_on_exit_zero`] for "pass on
    /// non-zero" (useful for negative assertions like "this should still
    /// fail to compile").
    ShellCommand {
        command: String,
        #[serde(default = "default_true")]
        pass_on_exit_zero: bool,
    },
    /// Pass if the given path exists in the workspace.
    FileExists { path: String },
    /// Pass if the given file's contents match both (all supplied) of:
    /// a substring, a regex. At least one of the two must be provided.
    FileContains {
        path: String,
        #[serde(default)]
        substring: Option<String>,
        /// Serialised as `regex = "…"` in TOML.
        #[serde(default)]
        regex: Option<String>,
    },
    /// All children must pass.
    All { graders: Vec<GraderSpec> },
    /// At least one child must pass.
    Any { graders: Vec<GraderSpec> },
}

fn default_true() -> bool {
    true
}

/// Result of grading a single rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradeOutcome {
    pub passed: bool,
    /// Human-readable detail — what the grader did and why it
    /// passed/failed. Always populated (never empty) so log output is
    /// uniformly informative.
    pub detail: String,
}

impl GradeOutcome {
    pub fn passed(detail: impl Into<String>) -> Self {
        Self {
            passed: true,
            detail: detail.into(),
        }
    }

    pub fn failed(detail: impl Into<String>) -> Self {
        Self {
            passed: false,
            detail: detail.into(),
        }
    }
}

impl GraderSpec {
    /// Evaluate this rule against the provided environment.
    ///
    /// Grading is deliberately synchronous: the common case is a quick
    /// filesystem check or a short command. If long-running commands
    /// become common, switch to spawning and polling — but the MVP keeps
    /// things simple.
    pub fn grade(&self, env: &EvalEnvironment) -> GradeOutcome {
        match self {
            GraderSpec::ShellCommand {
                command,
                pass_on_exit_zero,
            } => grade_shell(env, command, *pass_on_exit_zero),
            GraderSpec::FileExists { path } => grade_file_exists(env, path),
            GraderSpec::FileContains {
                path,
                substring,
                regex,
            } => grade_file_contains(env, path, substring.as_deref(), regex.as_deref()),
            GraderSpec::All { graders } => grade_all(env, graders),
            GraderSpec::Any { graders } => grade_any(env, graders),
        }
    }

    /// Short one-line description. Useful for logs and report headers.
    pub fn describe(&self) -> String {
        match self {
            GraderSpec::ShellCommand {
                command,
                pass_on_exit_zero,
            } => {
                let expect = if *pass_on_exit_zero { "=0" } else { "≠0" };
                format!("shell[{expect}]: {command}")
            }
            GraderSpec::FileExists { path } => format!("file_exists: {path}"),
            GraderSpec::FileContains {
                path,
                substring,
                regex,
            } => {
                let mut parts = Vec::new();
                if let Some(s) = substring {
                    parts.push(format!("contains {s:?}"));
                }
                if let Some(r) = regex {
                    parts.push(format!("matches /{r}/"));
                }
                format!("file_contains({path}): {}", parts.join(" AND "))
            }
            GraderSpec::All { graders } => format!("all_of({})", graders.len()),
            GraderSpec::Any { graders } => format!("any_of({})", graders.len()),
        }
    }
}

fn grade_shell(env: &EvalEnvironment, command: &str, pass_on_zero: bool) -> GradeOutcome {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd.current_dir(&env.workspace_path);
    for (k, v) in &env.env_vars {
        cmd.env(k, v);
    }

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return GradeOutcome::failed(format!("failed to spawn `sh -c`: {e}"));
        }
    };

    let exit = output.status.code().unwrap_or(-1);
    let zero = exit == 0;
    let passed = if pass_on_zero { zero } else { !zero };
    let detail = format!(
        "`{command}` exited {exit}{} (expected {})",
        if output.stderr.is_empty() {
            "".into()
        } else {
            format!(
                ", stderr: {}",
                truncate(&String::from_utf8_lossy(&output.stderr), 200)
            )
        },
        if pass_on_zero { "0" } else { "non-zero" }
    );
    GradeOutcome {
        passed,
        detail,
    }
}

fn grade_file_exists(env: &EvalEnvironment, path: &str) -> GradeOutcome {
    let full = env.workspace_path.join(path);
    if full.exists() {
        GradeOutcome::passed(format!("{path} exists"))
    } else {
        GradeOutcome::failed(format!("{path} does not exist"))
    }
}

fn grade_file_contains(
    env: &EvalEnvironment,
    path: &str,
    substring: Option<&str>,
    regex_src: Option<&str>,
) -> GradeOutcome {
    if substring.is_none() && regex_src.is_none() {
        return GradeOutcome::failed(
            "file_contains requires at least one of `substring` or `regex`".to_string(),
        );
    }
    let full = env.workspace_path.join(path);
    let contents = match fs::read_to_string(&full) {
        Ok(c) => c,
        Err(e) => {
            return GradeOutcome::failed(format!("failed to read {path}: {e}"));
        }
    };

    if let Some(s) = substring {
        if !contents.contains(s) {
            return GradeOutcome::failed(format!("{path} missing substring {s:?}"));
        }
    }
    if let Some(r) = regex_src {
        let re = match Regex::new(r) {
            Ok(re) => re,
            Err(e) => return GradeOutcome::failed(format!("invalid regex /{r}/: {e}")),
        };
        if !re.is_match(&contents) {
            return GradeOutcome::failed(format!("{path} does not match /{r}/"));
        }
    }
    GradeOutcome::passed(format!("{path} matches all constraints"))
}

fn grade_all(env: &EvalEnvironment, graders: &[GraderSpec]) -> GradeOutcome {
    for g in graders {
        let outcome = g.grade(env);
        if !outcome.passed {
            return GradeOutcome::failed(format!("all_of failed on `{}`: {}", g.describe(), outcome.detail));
        }
    }
    GradeOutcome::passed(format!("all {} rules passed", graders.len()))
}

fn grade_any(env: &EvalEnvironment, graders: &[GraderSpec]) -> GradeOutcome {
    let mut reasons = Vec::with_capacity(graders.len());
    for g in graders {
        let outcome = g.grade(env);
        if outcome.passed {
            return GradeOutcome::passed(format!("any_of passed via `{}`", g.describe()));
        }
        reasons.push(format!("{}: {}", g.describe(), outcome.detail));
    }
    GradeOutcome::failed(format!("all {} rules failed: {}", graders.len(), reasons.join("; ")))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut out = s[..max].to_string();
        out.push('…');
        out
    }
}
