//! Reaction Engine Bridge
//!
//! Connects the reaction engine to the orchestrator lifecycle.
//! Converts pipeline results and external events into `ReactionEvent`s
//! and dispatches them through the `ReactionEngine` for autonomous handling.

use std::sync::Arc;

use tracing::{info, warn};

use super::super::engine::PipelineRunResult;
use super::super::reaction::{ReactionConfig, ReactionEngine, ReactionEvent, ReactionType};
use super::orchestrator::PipelineOrchestrator;

/// Bridges the orchestrator's pipeline results into reaction engine events.
pub struct ReactionBridge {
    engine: Arc<ReactionEngine>,
}

impl ReactionBridge {
    /// Create a new bridge with the given reaction config.
    pub fn new(config: ReactionConfig) -> Self {
        Self {
            engine: Arc::new(ReactionEngine::with_config(config)),
        }
    }

    /// Create a disabled bridge (all reactions are no-ops).
    pub fn disabled() -> Self {
        Self {
            engine: Arc::new(ReactionEngine::with_config(ReactionConfig::disabled())),
        }
    }

    /// Process a completed pipeline result and react accordingly.
    ///
    /// Called after `sync_github_task_finished`. If the task failed,
    /// this emits a `TaskFailed` event. If the task succeeded and raised
    /// a PR, this triggers CI monitoring.
    pub async fn on_task_completed(&self, result: &PipelineRunResult) -> ReactionOutcome {
        if result.success {
            self.handle_success(result).await
        } else {
            self.handle_failure(result).await
        }
    }

    /// Process an external CI failure event from GitHub.
    pub async fn on_ci_failure(&self, event: ReactionEvent) -> ReactionOutcome {
        self.dispatch(event).await
    }

    /// Process an external review comment event from GitHub.
    pub async fn on_review_comment(&self, event: ReactionEvent) -> ReactionOutcome {
        self.dispatch(event).await
    }

    /// Process a merge conflict event.
    pub async fn on_merge_conflict(&self, event: ReactionEvent) -> ReactionOutcome {
        self.dispatch(event).await
    }

    /// Access the underlying engine for stats or audit queries.
    pub fn engine(&self) -> &Arc<ReactionEngine> {
        &self.engine
    }

    // -- Private helpers -------------------------------------------------------

    async fn handle_success(&self, result: &PipelineRunResult) -> ReactionOutcome {
        info!(
            "Task {} completed successfully, no reaction needed",
            result.task.id
        );
        ReactionOutcome::NoAction
    }

    async fn handle_failure(&self, result: &PipelineRunResult) -> ReactionOutcome {
        let failed_phase = result.task.phase;
        let error = result
            .error
            .clone()
            .unwrap_or_else(|| "unknown error".to_string());

        info!(
            "Task {} failed at phase {:?}: {}",
            result.task.id, failed_phase, error
        );

        let event = ReactionEvent::TaskFailed {
            task_id: result.task.id.clone(),
            error,
            failed_phase: Some(failed_phase),
            retry_count: 0,
        };

        self.dispatch(event).await
    }

    async fn dispatch(&self, event: ReactionEvent) -> ReactionOutcome {
        let reaction_result = self.engine.process_event(event).await;

        info!(
            "Reaction: {:?} for {} — {}",
            reaction_result.reaction,
            reaction_result.event.event_type(),
            reaction_result.reason
        );

        match reaction_result.reaction {
            ReactionType::AutoFix => ReactionOutcome::AutoFixRequested {
                task_id: reaction_result.event.task_id().map(|s| s.to_string()),
                reason: reaction_result.reason,
            },
            ReactionType::Notify => ReactionOutcome::Notify {
                task_id: reaction_result.event.task_id().map(|s| s.to_string()),
                reason: reaction_result.reason,
            },
            ReactionType::Escalate => ReactionOutcome::Escalate {
                task_id: reaction_result.event.task_id().map(|s| s.to_string()),
                reason: reaction_result.reason,
            },
            ReactionType::Checkpoint => ReactionOutcome::Checkpoint {
                task_id: reaction_result.event.task_id().map(|s| s.to_string()),
                reason: reaction_result.reason,
            },
            ReactionType::Cancel => ReactionOutcome::Cancel {
                task_id: reaction_result.event.task_id().map(|s| s.to_string()),
                reason: reaction_result.reason,
            },
            ReactionType::NoAction => ReactionOutcome::NoAction,
        }
    }
}

/// Outcome of a reaction dispatch — tells the orchestrator what to do next.
#[derive(Debug)]
pub enum ReactionOutcome {
    /// No action needed (task succeeded or reaction disabled).
    NoAction,
    /// Auto-fix was requested — the orchestrator should re-queue the task.
    AutoFixRequested {
        task_id: Option<String>,
        reason: String,
    },
    /// Human notification was triggered.
    Notify {
        task_id: Option<String>,
        reason: String,
    },
    /// Escalation requested — increase priority or alert.
    Escalate {
        task_id: Option<String>,
        reason: String,
    },
    /// Checkpoint the task state for recovery.
    Checkpoint {
        task_id: Option<String>,
        reason: String,
    },
    /// Task should be cancelled.
    Cancel {
        task_id: Option<String>,
        reason: String,
    },
}

impl ReactionOutcome {
    /// Extract the task ID if present.
    pub fn task_id(&self) -> Option<&str> {
        match self {
            Self::NoAction => None,
            Self::AutoFixRequested { task_id, .. }
            | Self::Notify { task_id, .. }
            | Self::Escalate { task_id, .. }
            | Self::Checkpoint { task_id, .. }
            | Self::Cancel { task_id, .. } => task_id.as_deref(),
        }
    }
}

/// Execute the reaction outcome against the orchestrator.
///
/// This is a standalone function (not a method) to keep the bridge
/// decoupled from the orchestrator's concrete type.
pub async fn execute_outcome(
    orchestrator: &PipelineOrchestrator,
    outcome: &ReactionOutcome,
) {
    match outcome {
        ReactionOutcome::AutoFixRequested { task_id, reason } => {
            if let Some(id) = task_id {
                info!("Re-queuing task {} for auto-fix: {}", id, reason);
                if let Err(e) = orchestrator.requeue_task(id).await {
                    warn!("Failed to re-queue task {} for auto-fix: {}", id, e);
                }
            }
        }
        ReactionOutcome::Cancel { task_id, reason } => {
            if let Some(id) = task_id {
                info!("Cancelling task {}: {}", id, reason);
                if let Err(e) = orchestrator.cancel_task(id).await {
                    warn!("Failed to cancel task {}: {}", id, e);
                }
            }
        }
        ReactionOutcome::Checkpoint { task_id, reason } => {
            info!("Checkpoint requested for task {:?}: {}", task_id, reason);
            // Checkpointing is handled by the recovery manager automatically
        }
        ReactionOutcome::Escalate { reason, .. } => {
            warn!("Escalation requested: {}", reason);
        }
        ReactionOutcome::Notify { reason, .. } => {
            info!("Notification: {}", reason);
            // Notification wiring will be added in a separate commit
        }
        ReactionOutcome::NoAction => {}
    }
}
