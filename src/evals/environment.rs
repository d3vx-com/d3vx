//! Isolated workspace provisioning for eval runs.
//!
//! Each task runs in a fresh directory under `~/.d3vx/evals/{run_id}/`
//! so setup commands and agent edits can't contaminate the host
//! checkout. The directory is deliberately left behind on failure —
//! deleting it would destroy the evidence an operator needs to debug
//! why a task failed. Callers that want eager cleanup call
//! [`EvalEnvironment::cleanup`] themselves.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::task::EvalTask;

/// An isolated workspace for running one evaluation task.
#[derive(Debug)]
pub struct EvalEnvironment {
    /// Unique run identifier (used as directory name).
    pub id: String,
    /// Root of the sandbox. Setup commands and agent work happen here.
    pub workspace_path: PathBuf,
    /// Environment variables to set when the harness spawns subcommands
    /// (setup steps, shell graders). The map is cloned into each spawn.
    pub env_vars: HashMap<String, String>,
}

impl EvalEnvironment {
    /// Provision an environment for `task` rooted at `root` (typically
    /// `~/.d3vx/evals`). Runs the task's setup commands in order; the
    /// first failing setup aborts provisioning and returns an error.
    pub fn provision(
        task: &EvalTask,
        root: impl AsRef<Path>,
    ) -> Result<Self, EnvironmentError> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|source| EnvironmentError::Io {
            path: root.to_path_buf(),
            source,
        })?;

        let id = format!("{}-{}", task.id, unique_suffix());
        let workspace = root.join(&id);
        fs::create_dir_all(&workspace).map_err(|source| EnvironmentError::Io {
            path: workspace.clone(),
            source,
        })?;

        let mut env = Self {
            id,
            workspace_path: workspace,
            env_vars: HashMap::new(),
        };
        env.run_setup(&task.setup)?;
        Ok(env)
    }

    /// Construct an environment without provisioning — callers supply
    /// an already-prepared directory. Used by tests and by integrations
    /// that manage their own workspace layout.
    pub fn adopt(id: impl Into<String>, workspace_path: PathBuf) -> Self {
        Self {
            id: id.into(),
            workspace_path,
            env_vars: HashMap::new(),
        }
    }

    /// Add an env var applied to every subcommand the harness spawns.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Remove the workspace directory. Call once all grading is done
    /// and the caller no longer needs the artefacts.
    pub fn cleanup(self) -> Result<(), EnvironmentError> {
        if self.workspace_path.exists() {
            fs::remove_dir_all(&self.workspace_path).map_err(|source| {
                EnvironmentError::Io {
                    path: self.workspace_path.clone(),
                    source,
                }
            })?;
        }
        Ok(())
    }

    fn run_setup(&mut self, steps: &[String]) -> Result<(), EnvironmentError> {
        use std::process::Command;
        for (idx, step) in steps.iter().enumerate() {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(step);
            cmd.current_dir(&self.workspace_path);
            for (k, v) in &self.env_vars {
                cmd.env(k, v);
            }
            let status = cmd.status().map_err(|source| EnvironmentError::SetupSpawn {
                step_index: idx,
                step: step.clone(),
                source,
            })?;
            if !status.success() {
                return Err(EnvironmentError::SetupFailed {
                    step_index: idx,
                    step: step.clone(),
                    exit: status.code().unwrap_or(-1),
                });
            }
        }
        Ok(())
    }
}

/// Produce a short unique suffix from wall-clock + a counter so two
/// environments provisioned within the same millisecond still get
/// distinct directories.
fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{millis:x}-{seq:x}")
}

/// Errors raised while provisioning or tearing down an environment.
#[derive(Debug, Error)]
pub enum EnvironmentError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not spawn setup step #{step_index} (`{step}`): {source}")]
    SetupSpawn {
        step_index: usize,
        step: String,
        #[source]
        source: std::io::Error,
    },

    #[error("setup step #{step_index} (`{step}`) exited {exit}")]
    SetupFailed {
        step_index: usize,
        step: String,
        exit: i32,
    },
}
