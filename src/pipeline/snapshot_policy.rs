//! Snapshot Policy
//!
//! Decides when to take session snapshots during pipeline execution.
//!
//! Snapshots are triggered by events, not polling:
//! - Phase completion
//! - Error during phase execution
//! - Periodic heartbeat (every N seconds)
//! - Task state changes
//!
//! The policy is configurable and maintains its own cooldown to avoid
//! excessive disk writes (deduplicates rapid-fire events).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tracing::debug;

use crate::pipeline::checkpoint::CheckpointManager;
use crate::pipeline::phases::Phase;
use crate::pipeline::resume::{ResumeManager, SessionSnapshot};

/// Configuration for when snapshots should be taken.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Take snapshot after each phase completes
    pub on_phase_complete: bool,
    /// Take snapshot when a phase fails
    pub on_error: bool,
    /// Take snapshot at regular intervals (0 = disabled)
    pub heartbeat_interval_secs: u64,
    /// Minimum time between snapshots (dedup cooldown)
    pub min_interval_secs: u64,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            on_phase_complete: true,
            on_error: true,
            heartbeat_interval_secs: 300, // every 5 minutes
            min_interval_secs: 10,        // at most 1 every 10s
        }
    }
}

/// Events that can trigger a snapshot.
#[derive(Debug, Clone)]
pub enum SnapshotTrigger {
    /// A phase completed successfully
    PhaseComplete(Phase),
    /// A phase execution failed
    PhaseError(Phase, String),
    /// Periodic heartbeat timer elapsed
    Heartbeat,
    /// Task state changed
    TaskTransition(&'static str),
}

/// Policy engine for automatic session snapshots.
///
/// Maintains a cooldown timer and decides whether to take a snapshot
/// based on the configured policy.
pub struct SnapshotPolicy {
    config: SnapshotConfig,
    last_snapshot: Option<Instant>,
    #[allow(dead_code)]
    resume_manager: Option<ResumeManager>,
    #[allow(dead_code)]
    checkpoint_manager: CheckpointManager,
}

impl SnapshotPolicy {
    pub fn new(config: SnapshotConfig, resume_manager: Option<ResumeManager>) -> Self {
        Self {
            config,
            last_snapshot: None,
            resume_manager,
            checkpoint_manager: CheckpointManager::new(PathBuf::from(".d3vx-checkpoints")),
        }
    }

    /// Check if a snapshot should be triggered for the given event.
    ///
    /// Returns `true` if a snapshot was actually taken.
    pub async fn should_snapshot(
        &mut self,
        trigger: &SnapshotTrigger,
        session_snap: Option<&SessionSnapshot>,
    ) -> bool {
        // Check if this trigger type is enabled
        if !self.is_trigger_enabled(trigger) {
            return false;
        }

        // Check cooldown
        if !self.cooldown_elapsed() {
            debug!("Snapshot skipped — cooldown still active");
            return false;
        }

        // Take snapshot if we have the infrastructure
        if let Some(snapshot) = session_snap {
            if let Some(ref mgr) = self.resume_manager {
                if let Err(e) = mgr.save_snapshot(snapshot).await {
                    debug!(error = %e, "Failed to save snapshot");
                    return false;
                }
                self.last_snapshot = Some(Instant::now());
                debug!(
                    "Snapshot taken — trigger: {:?}",
                    trigger_type_label(trigger)
                );
                return true;
            }
        }

        false
    }

    /// Note a checkpoint-worthy event.
    ///
    /// Checkpoints are already persisted by `CheckpointManager` during
    /// phase execution in the pipeline engine. This method is a hook
    /// point for the engine to signal a checkpoint-worthy event.
    pub fn note_checkpoint(&mut self, trigger: &SnapshotTrigger) {
        if let SnapshotTrigger::PhaseComplete(phase) = trigger {
            debug!(?phase, "Checkpoint noted for phase completion");
        }
    }

    /// Get the heartbeat interval for spawning a timer task.
    pub fn heartbeat_interval(&self) -> Option<Duration> {
        if self.config.heartbeat_interval_secs > 0 {
            Some(Duration::from_secs(self.config.heartbeat_interval_secs))
        } else {
            None
        }
    }

    /// Spawn a background heartbeat that sends triggers on the channel.
    pub fn spawn_heartbeat(
        &self,
        tx: mpsc::Sender<SnapshotTrigger>,
    ) -> tokio::task::JoinHandle<()> {
        let interval = self.heartbeat_interval();
        tokio::spawn(async move {
            if let Some(dur) = interval {
                let mut ticker = tokio::time::interval(dur);
                loop {
                    ticker.tick().await;
                    if tx.send(SnapshotTrigger::Heartbeat).await.is_err() {
                        break;
                    }
                }
            }
        })
    }

    /// Check if a trigger type is enabled by config.
    fn is_trigger_enabled(&self, trigger: &SnapshotTrigger) -> bool {
        match trigger {
            SnapshotTrigger::PhaseComplete(_) => self.config.on_phase_complete,
            SnapshotTrigger::PhaseError(_, _) => self.config.on_error,
            SnapshotTrigger::Heartbeat => self.config.heartbeat_interval_secs > 0,
            SnapshotTrigger::TaskTransition(_) => true, // always allowed
        }
    }

    /// Check if cooldown has elapsed since the last snapshot.
    fn cooldown_elapsed(&self) -> bool {
        match self.last_snapshot {
            Some(instant) => instant.elapsed().as_secs() >= self.config.min_interval_secs,
            None => true, // never snapped, always eligible
        }
    }
}

fn trigger_type_label(trigger: &SnapshotTrigger) -> &'static str {
    match trigger {
        SnapshotTrigger::PhaseComplete(_) => "phase_complete",
        SnapshotTrigger::PhaseError(_, _) => "phase_error",
        SnapshotTrigger::Heartbeat => "heartbeat",
        SnapshotTrigger::TaskTransition(_) => "task_transition",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::resume::types::{SerializedMessage, ToolRecord};

    fn test_snapshot() -> SessionSnapshot {
        SessionSnapshot {
            session_id: "test".to_string(),
            task_id: "t1".to_string(),
            snapshot_at: chrono::Utc::now(),
            messages: vec![SerializedMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: None,
            }],
            current_phase: "research".to_string(),
            modified_files: vec![],
            tool_history: vec![ToolRecord {
                tool_name: "Read".to_string(),
                input_summary: "file.rs".to_string(),
                success: true,
                timestamp: chrono::Utc::now(),
            }],
            checkpoint_note: None,
            event_log: None,
        }
    }

    #[test]
    fn test_default_config_enables_phase_and_error() {
        let config = SnapshotConfig::default();
        assert!(config.on_phase_complete);
        assert!(config.on_error);
        assert!(config.heartbeat_interval_secs > 0);
    }

    #[test]
    fn test_cooldown_initial_allows() {
        let policy = SnapshotPolicy::new(SnapshotConfig::default(), None);
        assert!(policy.cooldown_elapsed());
    }

    #[test]
    fn test_heartbeat_interval() {
        let policy = SnapshotPolicy::new(SnapshotConfig::default(), None);
        assert!(policy.heartbeat_interval().is_some());

        let no_heartbeat = SnapshotConfig {
            heartbeat_interval_secs: 0,
            ..SnapshotConfig::default()
        };
        let policy = SnapshotPolicy::new(no_heartbeat, None);
        assert!(policy.heartbeat_interval().is_none());
    }

    #[test]
    fn test_disabled_trigger_rejected() {
        let config = SnapshotConfig {
            on_phase_complete: false,
            on_error: false,
            heartbeat_interval_secs: 0,
            min_interval_secs: 10,
        };
        let policy = SnapshotPolicy::new(config, None);

        assert!(!policy.is_trigger_enabled(&SnapshotTrigger::PhaseComplete(Phase::Research)));
        assert!(!policy.is_trigger_enabled(&SnapshotTrigger::Heartbeat));
    }

    #[tokio::test]
    async fn test_no_resume_manager_returns_false() {
        let mut policy = SnapshotPolicy::new(SnapshotConfig::default(), None);
        let snap = test_snapshot();
        let result = policy
            .should_snapshot(
                &SnapshotTrigger::PhaseComplete(Phase::Research),
                Some(&snap),
            )
            .await;
        assert!(!result);
    }
}
