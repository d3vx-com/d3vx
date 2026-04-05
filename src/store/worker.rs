//! Worker store for managing agent processes
//!
//! Workers are agent instances that execute tasks. This module
//! tracks worker status, heartbeats, and run assignments.

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::database::{Database, DatabaseError};

/// Type of worker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerType {
    /// Standard agent
    Agent,
    /// Vex execution worker
    Vex,
    /// Background daemon
    Daemon,
}

impl std::fmt::Display for WorkerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerType::Agent => write!(f, "agent"),
            WorkerType::Vex => write!(f, "vex"),
            WorkerType::Daemon => write!(f, "daemon"),
        }
    }
}

impl std::str::FromStr for WorkerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "AGENT" => Ok(WorkerType::Agent),
            "VEX" => Ok(WorkerType::Vex),
            "DAEMON" => Ok(WorkerType::Daemon),
            _ => Err(format!("Invalid worker type: {}", s)),
        }
    }
}

/// Status of a worker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    /// Available for work
    Idle,
    /// Currently executing a task
    Busy,
    /// Offline/unavailable
    Offline,
    /// Error state
    Error,
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatus::Idle => write!(f, "IDLE"),
            WorkerStatus::Busy => write!(f, "BUSY"),
            WorkerStatus::Offline => write!(f, "OFFLINE"),
            WorkerStatus::Error => write!(f, "ERROR"),
        }
    }
}

impl std::str::FromStr for WorkerStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "IDLE" => Ok(WorkerStatus::Idle),
            "BUSY" => Ok(WorkerStatus::Busy),
            "OFFLINE" => Ok(WorkerStatus::Offline),
            "ERROR" => Ok(WorkerStatus::Error),
            _ => Err(format!("Invalid worker status: {}", s)),
        }
    }
}

/// An agent worker process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    /// Unique worker identifier
    pub id: String,
    /// Type of worker
    pub worker_type: WorkerType,
    /// Current status
    pub status: WorkerStatus,
    /// Currently assigned run ID
    pub current_run_id: Option<String>,
    /// Last heartbeat timestamp
    pub last_heartbeat_at: Option<String>,
    /// Worker capabilities (JSON)
    pub capabilities_json: String,
    /// Creation timestamp
    pub created_at: String,
}

/// Input for registering a new worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterWorker {
    /// Optional custom ID
    pub id: Option<String>,
    /// Worker type
    pub worker_type: Option<WorkerType>,
    /// Initial capabilities
    pub capabilities: Option<serde_json::Value>,
}

/// Options for listing workers
#[derive(Debug, Clone, Default)]
pub struct WorkerListOptions {
    /// Filter by status
    pub status: Option<WorkerStatus>,
    /// Filter by type
    pub worker_type: Option<WorkerType>,
    /// Maximum results
    pub limit: Option<usize>,
}

/// Worker store for CRUD operations
pub struct WorkerStore<'a> {
    conn: &'a Connection,
}

impl<'a> WorkerStore<'a> {
    /// Create a new worker store
    pub fn new(db: &'a Database) -> Self {
        Self {
            conn: db.connection(),
        }
    }

    /// Create a new worker store from a connection
    pub fn from_connection(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Register a new worker
    pub fn register(&self, input: RegisterWorker) -> Result<Worker, DatabaseError> {
        let now = crate::store::now_iso();
        let id = input
            .id
            .unwrap_or_else(|| crate::store::generate_id("worker"));

        let capabilities_json =
            serde_json::to_string(&input.capabilities.unwrap_or(serde_json::json!({})))
                .unwrap_or_else(|_| "{}".to_string());

        let worker = Worker {
            id: id.clone(),
            worker_type: input.worker_type.unwrap_or(WorkerType::Agent),
            status: WorkerStatus::Idle,
            current_run_id: None,
            last_heartbeat_at: Some(now.clone()),
            capabilities_json,
            created_at: now,
        };

        self.conn
            .execute(
                "INSERT INTO workers (id, worker_type, status, current_run_id, last_heartbeat_at, capabilities_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    worker.id,
                    worker.worker_type.to_string(),
                    worker.status.to_string(),
                    worker.current_run_id,
                    worker.last_heartbeat_at,
                    worker.capabilities_json,
                    worker.created_at,
                ],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker registered: {} ({:?})", id, worker.worker_type);
        Ok(worker)
    }

    /// Get a worker by ID
    pub fn get(&self, id: &str) -> Result<Option<Worker>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT * FROM workers WHERE id = ?1",
            params![id],
            Self::row_to_worker,
        );

        match result {
            Ok(worker) => Ok(Some(worker)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e)),
        }
    }

    /// Update worker heartbeat
    pub fn update_heartbeat(&self, id: &str) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        self.conn
            .execute(
                "UPDATE workers SET last_heartbeat_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker heartbeat updated: {}", id);
        Ok(())
    }

    /// Assign a run to a worker
    pub fn assign_run(&self, worker_id: &str, run_id: &str) -> Result<(), DatabaseError> {
        let now = crate::store::now_iso();

        self.conn
            .execute(
                "UPDATE workers SET status = 'BUSY', current_run_id = ?1, last_heartbeat_at = ?2 WHERE id = ?3",
                params![run_id, now, worker_id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker {} assigned to run {}", worker_id, run_id);
        Ok(())
    }

    /// Release a worker from its current run
    pub fn release_run(&self, worker_id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute(
                "UPDATE workers SET status = 'IDLE', current_run_id = NULL WHERE id = ?1",
                params![worker_id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker {} released from run", worker_id);
        Ok(())
    }

    /// Get all idle workers
    pub fn get_idle_workers(&self) -> Result<Vec<Worker>, DatabaseError> {
        self.query_workers(
            "SELECT * FROM workers WHERE status = 'IDLE' ORDER BY created_at ASC",
            params![],
        )
    }

    /// Get all active workers (busy or recently active)
    pub fn get_active_workers(&self) -> Result<Vec<Worker>, DatabaseError> {
        self.query_workers(
            "SELECT * FROM workers WHERE status IN ('IDLE', 'BUSY') ORDER BY last_heartbeat_at DESC",
            params![],
        )
    }

    /// Get stale workers (no heartbeat for a while)
    pub fn get_stale_workers(&self, timeout_seconds: i64) -> Result<Vec<Worker>, DatabaseError> {
        self.query_workers(
            "SELECT * FROM workers WHERE status = 'BUSY' AND datetime(last_heartbeat_at) < datetime('now', ?1 || ' seconds')",
            params![format!("-{}", timeout_seconds)],
        )
    }

    /// Mark a worker as offline
    pub fn mark_offline(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute(
                "UPDATE workers SET status = 'OFFLINE', current_run_id = NULL WHERE id = ?1",
                params![id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker marked offline: {}", id);
        Ok(())
    }

    /// Mark a worker as having an error
    pub fn mark_error(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute(
                "UPDATE workers SET status = 'ERROR' WHERE id = ?1",
                params![id],
            )
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker marked error: {}", id);
        Ok(())
    }

    /// List workers with filtering
    pub fn list(&self, options: WorkerListOptions) -> Result<Vec<Worker>, DatabaseError> {
        let limit = options.limit.unwrap_or(100);

        let workers = if let Some(status) = &options.status {
            self.query_workers(
                "SELECT * FROM workers WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
                params![status.to_string(), limit as i64],
            )?
        } else if let Some(worker_type) = &options.worker_type {
            self.query_workers(
                "SELECT * FROM workers WHERE worker_type = ?1 ORDER BY created_at DESC LIMIT ?2",
                params![worker_type.to_string(), limit as i64],
            )?
        } else {
            self.query_workers(
                "SELECT * FROM workers ORDER BY created_at DESC LIMIT ?1",
                params![limit as i64],
            )?
        };

        Ok(workers)
    }

    /// Delete a worker
    pub fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute("DELETE FROM workers WHERE id = ?1", params![id])
            .map_err(DatabaseError::QueryError)?;

        debug!("Worker deleted: {}", id);
        Ok(())
    }

    /// Helper to query workers
    fn query_workers<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<Worker>, DatabaseError> {
        let mut stmt = self.conn.prepare(sql).map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map(params, Self::row_to_worker)
            .map_err(DatabaseError::QueryError)?;

        let mut workers = Vec::new();
        for row in rows {
            workers.push(row.map_err(DatabaseError::QueryError)?);
        }

        Ok(workers)
    }

    /// Map a database row to a Worker
    fn row_to_worker(row: &Row<'_>) -> rusqlite::Result<Worker> {
        let worker_type_str: String = row.get("worker_type")?;
        let status_str: String = row.get("status")?;

        Ok(Worker {
            id: row.get("id")?,
            worker_type: worker_type_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            status: status_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            current_run_id: row.get("current_run_id")?,
            last_heartbeat_at: row.get("last_heartbeat_at")?,
            capabilities_json: row.get("capabilities_json")?,
            created_at: row.get("created_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::run::TaskRunStore;
    use super::super::task::{NewTask, TaskStore};
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().expect("Failed to create in-memory database")
    }

    /// Helper to create a parent task + run for FK constraint
    fn create_parent_task_and_run(db: &Database, task_id: &str, run_id: &str) {
        let task_store = TaskStore::new(db);
        task_store
            .create(NewTask {
                id: Some(task_id.to_string()),
                title: "Parent task".to_string(),
                ..Default::default()
            })
            .expect("Failed to create parent task");

        let run_store = TaskRunStore::new(db);
        run_store
            .create(super::super::run::NewTaskRun {
                id: Some(run_id.to_string()),
                task_id: task_id.to_string(),
                attempt_number: 1,
                worker_id: None,
                workspace_id: None,
                metrics: None,
            })
            .expect("Failed to create parent run");
    }

    #[test]
    fn test_register_worker() {
        let db = create_test_db();
        let store = WorkerStore::new(&db);

        let worker = store
            .register(RegisterWorker {
                id: Some("worker-001".to_string()),
                worker_type: Some(WorkerType::Agent),
                capabilities: None,
            })
            .expect("Failed to register worker");

        assert_eq!(worker.id, "worker-001");
        assert_eq!(worker.worker_type, WorkerType::Agent);
        assert_eq!(worker.status, WorkerStatus::Idle);
    }

    #[test]
    fn test_assign_and_release_run() {
        let db = create_test_db();
        create_parent_task_and_run(&db, "task-002", "run-001");
        let store = WorkerStore::new(&db);

        store
            .register(RegisterWorker {
                id: Some("worker-002".to_string()),
                worker_type: None,
                capabilities: None,
            })
            .expect("Failed to register worker");

        store
            .assign_run("worker-002", "run-001")
            .expect("Failed to assign run");

        let worker = store
            .get("worker-002")
            .expect("Failed to get")
            .expect("Not found");
        assert_eq!(worker.status, WorkerStatus::Busy);
        assert_eq!(worker.current_run_id, Some("run-001".to_string()));

        store
            .release_run("worker-002")
            .expect("Failed to release run");

        let worker = store
            .get("worker-002")
            .expect("Failed to get")
            .expect("Not found");
        assert_eq!(worker.status, WorkerStatus::Idle);
        assert!(worker.current_run_id.is_none());
    }

    #[test]
    fn test_heartbeat() {
        let db = create_test_db();
        let store = WorkerStore::new(&db);

        store
            .register(RegisterWorker {
                id: Some("worker-003".to_string()),
                worker_type: None,
                capabilities: None,
            })
            .expect("Failed to register worker");

        std::thread::sleep(std::time::Duration::from_millis(10));

        store
            .update_heartbeat("worker-003")
            .expect("Failed to update heartbeat");

        let worker = store
            .get("worker-003")
            .expect("Failed to get")
            .expect("Not found");
        assert!(worker.last_heartbeat_at.is_some());
    }
}
