//! Eval task definition and TOML loading.
//!
//! A task is a single reproducible unit of evaluation work: what the
//! agent is asked to do, how the workspace is set up, and how to judge
//! whether the agent succeeded.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::grader::GraderSpec;

/// A single evaluation task.
///
/// Designed to be authored in TOML — see the test fixtures for examples.
/// `id` is derived from the filename when loaded from disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalTask {
    /// Stable identifier. When loaded from a file, defaults to the file
    /// stem (e.g. `evals/fix-bug.toml` → `fix-bug`).
    #[serde(default)]
    pub id: String,

    /// Human-readable name. Falls back to `id` when absent.
    pub name: String,

    /// Optional longer description (what the task is checking, context).
    #[serde(default)]
    pub description: Option<String>,

    /// The prompt given to the agent.
    pub instruction: String,

    /// Shell commands run *before* the agent starts, in the eval
    /// environment's workspace directory. Use these to seed files,
    /// clone fixtures, or install scaffolding the agent will modify.
    #[serde(default)]
    pub setup: Vec<String>,

    /// Ordered grading rules. All must pass (implicit `All`) unless the
    /// caller uses an explicit `GraderSpec::Any` at the top level.
    #[serde(default)]
    pub graders: Vec<GraderSpec>,

    /// Optional per-task cost cap in USD. `None` means the harness uses
    /// the global default.
    #[serde(default)]
    pub budget_usd: Option<f64>,

    /// Optional cap on iterations before the agent is stopped.
    #[serde(default)]
    pub max_iterations: Option<u32>,

    /// Optional wall-clock timeout in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Free-form tags (e.g. `"bugfix"`, `"multi-file"`, `"needs-tests"`).
    /// Used to filter/bucket results.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl EvalTask {
    /// Load a single task from a TOML file. The task's `id` is set to
    /// the file stem when the TOML doesn't provide one explicitly.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, TaskLoadError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| TaskLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut task: EvalTask = toml::from_str(&contents).map_err(|source| {
            TaskLoadError::Parse {
                path: path.to_path_buf(),
                source,
            }
        })?;

        if task.id.is_empty() {
            task.id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
        }
        task.validate().map_err(|source| TaskLoadError::Invalid {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(task)
    }

    /// Load every `*.toml` file directly in `dir` (non-recursive). Files
    /// that fail to parse produce errors in the returned vec so one bad
    /// task doesn't hide the rest.
    ///
    /// Files are loaded in sorted-filename order for deterministic runs.
    pub fn load_from_dir(
        dir: impl AsRef<Path>,
    ) -> Result<Vec<Result<Self, TaskLoadError>>, TaskLoadError> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(TaskLoadError::Io {
                path: dir.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "eval directory does not exist",
                ),
            });
        }

        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(|source| TaskLoadError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false))
            .collect();
        entries.sort();

        Ok(entries.into_iter().map(Self::load_from_file).collect())
    }

    /// Check task-level invariants beyond what Serde can express.
    pub fn validate(&self) -> Result<(), TaskError> {
        if self.name.is_empty() {
            return Err(TaskError::EmptyField("name"));
        }
        if self.instruction.is_empty() {
            return Err(TaskError::EmptyField("instruction"));
        }
        if let Some(b) = self.budget_usd {
            if !b.is_finite() || b <= 0.0 {
                return Err(TaskError::NonPositive {
                    field: "budget_usd",
                    value: b.to_string(),
                });
            }
        }
        if let Some(0) = self.max_iterations {
            return Err(TaskError::NonPositive {
                field: "max_iterations",
                value: "0".to_string(),
            });
        }
        if let Some(0) = self.timeout_secs {
            return Err(TaskError::NonPositive {
                field: "timeout_secs",
                value: "0".to_string(),
            });
        }
        Ok(())
    }

    /// Display name, falling back to id when name is empty. Kept here so
    /// report rendering doesn't have to replicate the fallback.
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }

    /// True if the task's tags include `tag`. Case-sensitive.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// Errors raised by task-level invariant checks.
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("task field `{0}` must not be empty")]
    EmptyField(&'static str),

    #[error("task field `{field}` must be positive; got {value}")]
    NonPositive {
        field: &'static str,
        value: String,
    },
}

/// Errors raised when loading tasks from disk.
#[derive(Debug, Error)]
pub enum TaskLoadError {
    #[error("failed to read eval file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse eval file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("eval file {path} failed validation: {source}")]
    Invalid {
        path: PathBuf,
        #[source]
        source: TaskError,
    },
}
