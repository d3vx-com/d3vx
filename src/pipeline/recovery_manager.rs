//! Recovery Manager Module
//!
//! Handles background health checks, crash detection, and task recovery.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::phases::TaskStatus;
use super::queue_manager::QueueManager;
use crate::recovery::{CrashDetector, CrashStatus};
use crate::store::database::DatabaseHandle;
use crate::store::session::{SessionListOptions, SessionStore};

pub struct RecoveryManager {
    active_tasks: Arc<RwLock<HashMap<String, String>>>,
    queue_manager: Arc<QueueManager>,
    crash_detector: Arc<CrashDetector>,
    db: Option<DatabaseHandle>,
}

impl RecoveryManager {
    pub fn new(
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
        queue_manager: Arc<QueueManager>,
        crash_detector: Arc<CrashDetector>,
        db: Option<DatabaseHandle>,
    ) -> Self {
        Self {
            active_tasks,
            queue_manager,
            crash_detector,
            db,
        }
    }

    pub async fn start_watchdog(self: Arc<Self>) {
        let manager = self.clone();
        let interval = self.crash_detector.check_interval;

        info!(
            "Starting crash detection watchdog (interval: {:?})",
            interval
        );

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Err(e) = manager.check_all_tasks_health().await {
                    warn!("Watchdog health check failed: {}", e);
                }
            }
        });
    }

    async fn check_all_tasks_health(&self) -> Result<()> {
        let active = self.active_tasks.read().await.clone();
        let db_handle = match &self.db {
            Some(db) => db,
            None => return Ok(()),
        };

        for (task_id, _path) in active {
            let session = {
                let db = db_handle.lock();
                let store = SessionStore::from_connection(db.connection());
                store
                    .list(SessionListOptions {
                        task_id: Some(task_id.clone()),
                        ..Default::default()
                    })?
                    .into_iter()
                    .next()
            };

            if let Some(session) = session {
                let health = self.crash_detector.check_health(&session);
                match health {
                    CrashStatus::Crashed | CrashStatus::Unresponsive => {
                        warn!(
                            "Task {} session {} is {:?}. Triggering recovery...",
                            task_id, session.id, health
                        );
                        self.handle_task_failure(&task_id, &session).await?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn handle_task_failure(
        &self,
        task_id: &str,
        session: &crate::store::session::Session,
    ) -> Result<()> {
        self.queue_manager
            .transition_task(task_id, TaskStatus::Failed)
            .await?;
        self.active_tasks.write().await.remove(task_id);
        error!(
            "Watchdog detected failure for task {}. Session state: {:?}",
            task_id, session.state
        );
        Ok(())
    }
}
