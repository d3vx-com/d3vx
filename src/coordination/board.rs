//! Shared task board for coordinating parallel agents.
//!
//! Each task lives as one JSON file plus (optionally) a sibling
//! `.claim` file that records exclusive ownership. Status transitions
//! go through validated methods on [`CoordinationBoard`] — callers
//! cannot reach into the filesystem and bypass invariants.
//!
//! # Lifecycle
//!
//! ```text
//!   add_task → Pending
//!                │    claim_task(agent)
//!                ▼
//!             Claimed (owner = agent)
//!                │    complete_task / fail_task
//!                ▼
//!            Completed | Failed | Cancelled
//! ```
//!
//! The data shape (`BoardTask`, `TaskStatus`, `NewTask`) lives in
//! [`task`](super::task) — this file owns only the operations.

use std::path::{Path, PathBuf};

use chrono::Utc;

pub use super::task::{BoardTask, NewTask, TaskStatus};

use super::errors::CoordinationError;
use super::io;

/// The task board: operations on a directory of task JSON files plus
/// their claim siblings.
#[derive(Debug, Clone)]
pub struct CoordinationBoard {
    tasks_dir: PathBuf,
}

impl CoordinationBoard {
    /// Open (or create) a task board rooted at `tasks_dir`.
    pub fn open(tasks_dir: impl AsRef<Path>) -> Result<Self, CoordinationError> {
        let dir = tasks_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|source| CoordinationError::Io {
            path: dir.clone(),
            source,
        })?;
        Ok(Self { tasks_dir: dir })
    }

    pub fn root(&self) -> &Path {
        &self.tasks_dir
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir.join(format!("{id}.json"))
    }

    fn claim_path(&self, id: &str) -> PathBuf {
        self.tasks_dir.join(format!("{id}.claim"))
    }

    /// Add a task. Fails if an entry with the same id already exists —
    /// silently overwriting would lose history.
    pub fn add_task(&self, input: NewTask) -> Result<BoardTask, CoordinationError> {
        let path = self.task_path(&input.id);
        if path.exists() {
            return Err(CoordinationError::Io {
                path,
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("task `{}` already exists on the board", input.id),
                ),
            });
        }
        let now = Utc::now();
        let task = BoardTask {
            id: input.id,
            title: input.title,
            instruction: input.instruction,
            status: TaskStatus::Pending,
            owner: None,
            depends_on: input.depends_on,
            result: None,
            created_at: now,
            updated_at: now,
        };
        io::atomic_write_json(&path, &task)?;
        Ok(task)
    }

    /// Fetch a task by id. Returns `None` if the task is unknown.
    pub fn get_task(&self, id: &str) -> Result<Option<BoardTask>, CoordinationError> {
        io::read_json_if_exists(self.task_path(id))
    }

    /// List every task on the board, oldest-first by `created_at`.
    pub fn list_tasks(&self) -> Result<Vec<BoardTask>, CoordinationError> {
        let entries =
            std::fs::read_dir(&self.tasks_dir).map_err(|source| CoordinationError::Io {
                path: self.tasks_dir.clone(),
                source,
            })?;

        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Some(task) = io::read_json_if_exists::<BoardTask>(&path)? {
                out.push(task);
            }
        }
        out.sort_by_key(|t| t.created_at);
        Ok(out)
    }

    /// List tasks whose dependencies are all `Completed` and which are
    /// still `Pending` (no owner). Useful for workers asking "what can
    /// I claim right now?".
    pub fn list_ready_tasks(&self) -> Result<Vec<BoardTask>, CoordinationError> {
        let all = self.list_tasks()?;
        let completed: std::collections::HashSet<&str> = all
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.as_str())
            .collect();
        Ok(all
            .iter()
            .filter(|t| {
                t.status == TaskStatus::Pending
                    && t.owner.is_none()
                    && t.depends_on.iter().all(|d| completed.contains(d.as_str()))
            })
            .cloned()
            .collect())
    }

    /// Atomically claim a task for `agent_id`. Returns the updated task
    /// on success. Concurrent claimers will see `AlreadyClaimed` for
    /// the losing caller — the atomic primitive is POSIX
    /// `O_CREAT|O_EXCL` on the claim file.
    pub fn claim_task(
        &self,
        id: &str,
        agent_id: &str,
    ) -> Result<BoardTask, CoordinationError> {
        let mut task = self
            .get_task(id)?
            .ok_or_else(|| CoordinationError::TaskNotFound {
                task_id: id.to_string(),
            })?;

        if task.status != TaskStatus::Pending {
            return Err(CoordinationError::InvalidTransition {
                task_id: id.to_string(),
                from: task.status,
                to: TaskStatus::Claimed,
            });
        }

        let unresolved = self.unresolved_deps(&task)?;
        if !unresolved.is_empty() {
            return Err(CoordinationError::NotReady {
                task_id: id.to_string(),
                depends_on: task.depends_on.clone(),
                unresolved,
            });
        }

        // Atomic claim: first create wins. Any racer reading after sees
        // the current owner stamped in the claim file.
        let won = io::create_exclusive(self.claim_path(id), agent_id.as_bytes())?;
        if !won {
            let owner = std::fs::read_to_string(self.claim_path(id))
                .unwrap_or_else(|_| "<unknown>".to_string());
            return Err(CoordinationError::AlreadyClaimed {
                task_id: id.to_string(),
                owner,
            });
        }

        task.status = TaskStatus::Claimed;
        task.owner = Some(agent_id.to_string());
        task.updated_at = Utc::now();
        io::atomic_write_json(self.task_path(id), &task)?;
        Ok(task)
    }

    /// Mark a claimed task as completed. Caller MUST be the current
    /// owner — callers can verify via [`get_task`] first if needed.
    pub fn complete_task(
        &self,
        id: &str,
        result: impl Into<String>,
    ) -> Result<BoardTask, CoordinationError> {
        self.finish(id, TaskStatus::Completed, Some(result.into()))
    }

    /// Mark a claimed task as failed with a reason.
    pub fn fail_task(
        &self,
        id: &str,
        reason: impl Into<String>,
    ) -> Result<BoardTask, CoordinationError> {
        self.finish(id, TaskStatus::Failed, Some(reason.into()))
    }

    /// Mark a task (claimed or pending) as cancelled.
    pub fn cancel_task(&self, id: &str) -> Result<BoardTask, CoordinationError> {
        self.finish(id, TaskStatus::Cancelled, None)
    }

    fn finish(
        &self,
        id: &str,
        status: TaskStatus,
        result: Option<String>,
    ) -> Result<BoardTask, CoordinationError> {
        let mut task = self
            .get_task(id)?
            .ok_or_else(|| CoordinationError::TaskNotFound {
                task_id: id.to_string(),
            })?;
        if task.status.is_terminal() {
            return Err(CoordinationError::InvalidTransition {
                task_id: id.to_string(),
                from: task.status,
                to: status,
            });
        }
        task.status = status;
        task.result = result;
        task.updated_at = Utc::now();
        io::atomic_write_json(self.task_path(id), &task)?;
        // Release the claim once the task reaches a terminal state so
        // operators `cat`ing the dir see a clean state.
        let _ = std::fs::remove_file(self.claim_path(id));
        Ok(task)
    }

    fn unresolved_deps(&self, task: &BoardTask) -> Result<Vec<String>, CoordinationError> {
        let mut unresolved = Vec::new();
        for dep in &task.depends_on {
            match self.get_task(dep)? {
                Some(t) if t.status == TaskStatus::Completed => {}
                _ => unresolved.push(dep.clone()),
            }
        }
        Ok(unresolved)
    }
}
