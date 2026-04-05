//! Batch Child Status Updates
//!
//! Tracks in-progress child task status changes and persists
//! updates to the task store for parallel batch coordination.

use std::time::Instant;

use crate::app::{App, ParallelChildStatus};

impl App {
    pub(super) fn batch_merge_ready(batch: &crate::app::ParallelBatchState) -> bool {
        batch.children.iter().all(|child| {
            matches!(child.status, ParallelChildStatus::Completed)
                && child
                    .evaluation
                    .as_ref()
                    .map(|evaluation| {
                        evaluation.conflict_risk >= 3
                            && evaluation.test_lint_outcome >= 0
                            && evaluation.scope_adherence >= 2
                    })
                    .unwrap_or(false)
        })
    }

    pub(super) fn update_parallel_batch_child(
        &mut self,
        agent_id: &str,
        status: ParallelChildStatus,
        result: Option<String>,
    ) -> Option<String> {
        let mut changed_batch_id = None;
        let mut completed_batch_id = None;

        for batch in self.agents.parallel_batches.values_mut() {
            if let Some(child) = batch
                .children
                .iter_mut()
                .find(|child| child.agent_id.as_deref() == Some(agent_id))
            {
                child.status = status;
                child.result = result;
                child.evaluation = child
                    .result
                    .as_ref()
                    .map(|_| Self::evaluate_parallel_child(child));
                if let Some(task_id) = &child.task_id {
                    if let Some(db_handle) = &self.db {
                        let db = db_handle.lock();
                        let task_store =
                            crate::store::task::TaskStore::from_connection(db.connection());
                        let existing = task_store
                            .get(task_id)
                            .ok()
                            .flatten()
                            .map(|task| task.metadata)
                            .unwrap_or_else(|| "{}".to_string());
                        let merged = Self::merge_task_metadata(
                            &existing,
                            serde_json::json!({
                                "orchestration_node": {
                                    "status": format!("{:?}", status),
                                    "result": child.result,
                                    "evaluation": child.evaluation,
                                }
                            }),
                        );
                        let _ = task_store.update(
                            task_id,
                            crate::store::task::TaskUpdate {
                                state: Some(match status {
                                    ParallelChildStatus::Pending => {
                                        crate::store::task::TaskState::Queued
                                    }
                                    ParallelChildStatus::Running => {
                                        crate::store::task::TaskState::Spawning
                                    }
                                    ParallelChildStatus::Completed => {
                                        crate::store::task::TaskState::Done
                                    }
                                    ParallelChildStatus::Failed
                                    | ParallelChildStatus::Cancelled => {
                                        crate::store::task::TaskState::Failed
                                    }
                                }),
                                metadata: Some(merged),
                                ..Default::default()
                            },
                        );
                    }
                }
                changed_batch_id = Some(batch.id.clone());
                if batch.is_complete() {
                    batch.completed_at = Some(Instant::now());
                    completed_batch_id = Some(batch.id.clone());
                }
                break;
            }
        }

        if let Some(batch_id) = changed_batch_id {
            self.persist_parallel_batch_snapshot(&batch_id);
            return completed_batch_id;
        }

        None
    }
}
