//! Batch Spawn and Finalization
//!
//! Dependency-aware task scheduling, ready-task spawning, and batch
//! finalization including best-of-N selection and synthesis reporting.

use std::sync::Arc;

use crate::agent::{BestOfNConfig, BestOfNExecutor, VariantResult};
use crate::app::{App, ParallelChildStatus};

impl App {
    pub(super) fn is_batch_child_runnable(
        &self,
        batch_id: &str,
        task: &crate::tools::SpawnTask,
    ) -> bool {
        let Some(batch) = self.agents.parallel_batches.get(batch_id) else {
            return true;
        };

        task.depends_on.iter().all(|dependency| {
            batch.children.iter().any(|child| {
                child.key == *dependency && child.status == ParallelChildStatus::Completed
            })
        })
    }

    pub(super) async fn spawn_ready_parallel_tasks(
        &mut self,
        provider: Arc<dyn crate::providers::Provider>,
    ) {
        const MAX_CONCURRENT_AGENTS: usize = 5;

        loop {
            if self.agents.running_parallel_agents >= MAX_CONCURRENT_AGENTS {
                break;
            }

            let Some(queue_index) = self
                .agents
                .pending_agent_queue
                .iter()
                .position(|(batch_id, task)| self.is_batch_child_runnable(batch_id, task))
            else {
                break;
            };

            let (batch_id, task) = self.agents.pending_agent_queue.remove(queue_index);
            self.spawn_single_agent(&batch_id, &task, provider.clone())
                .await;
            self.agents.running_parallel_agents += 1;
        }
    }

    pub(super) async fn finalize_parallel_batch(&mut self, batch_id: &str) {
        let Some(mut batch) = self.agents.parallel_batches.get(batch_id).cloned() else {
            return;
        };

        // Best-of-N selection when requested
        if batch.select_best {
            if let Some(provider) = &self.provider {
                let completed_variants: Vec<(String, VariantResult)> = batch
                    .children
                    .iter()
                    .enumerate()
                    .filter_map(|(index, child)| {
                        child.result.as_ref().map(|result| {
                            (
                                child.key.clone(),
                                VariantResult {
                                    index,
                                    content: result.clone(),
                                    tokens: crate::providers::TokenUsage::default(),
                                    error: None,
                                },
                            )
                        })
                    })
                    .collect();

                if completed_variants.len() > 1 {
                    let evaluation_context = batch
                        .children
                        .iter()
                        .filter_map(|child| {
                            child.evaluation.as_ref().map(|evaluation| {
                                format!(
                                    "{} => total={} scope={} tests={} docs={} conflict={} files={}",
                                    child.key,
                                    evaluation.total_score,
                                    evaluation.scope_adherence,
                                    evaluation.test_lint_outcome,
                                    evaluation.docs_completeness,
                                    evaluation.conflict_risk,
                                    evaluation.changed_file_quality
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let selector_prompt = Some(format!(
                        "You are selecting the best candidate result from multiple agent implementations.\nPrioritize correctness, code quality, changed-file quality, test/lint outcomes, docs completeness, low conflict risk, and scope adherence.\n{}\nEvaluation context:\n{}",
                        batch.selection_criteria.as_deref().unwrap_or("Prefer the result that is safest to ship and easiest to review."),
                        evaluation_context
                    ));
                    let variants: Vec<VariantResult> = completed_variants
                        .iter()
                        .map(|(_, variant)| variant.clone())
                        .collect();
                    let executor =
                        BestOfNExecutor::with_config(provider.clone(), BestOfNConfig::default());
                    if let Ok((selected_index, selection_reasoning)) = executor
                        .select_existing_variants(
                            &batch.reasoning,
                            &variants,
                            selector_prompt.as_deref(),
                        )
                        .await
                    {
                        if let Some((selected_key, _)) = completed_variants.get(selected_index) {
                            batch.selected_child_key = Some(selected_key.clone());
                            batch.selection_reasoning = selection_reasoning.clone();
                            if let Some(stored_batch) =
                                self.agents.parallel_batches.get_mut(batch_id)
                            {
                                stored_batch.selected_child_key = Some(selected_key.clone());
                                stored_batch.selection_reasoning = selection_reasoning;
                            }
                        }
                    }
                }
            }
        }

        // File change report for each child in worktree mode
        let mut file_report = Vec::new();
        for child in &batch.children {
            if let Some(agent_id) = &child.agent_id {
                if let Some(handle) = self.subagents.get(agent_id).await {
                    if let Some(path) = &handle.worktree_path {
                        let output = std::process::Command::new("git")
                            .args(&["status", "--short"])
                            .current_dir(path)
                            .output();

                        if let Ok(o) = output {
                            let changes = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            if !changes.is_empty() {
                                file_report.push(format!(
                                    "**{}** changed files:\n```\n{}\n```",
                                    child.key, changes
                                ));
                            }
                        }
                    }
                }
            }
        }

        let mut synthesis_summary = Self::synthesize_parallel_batch(&batch)
            .unwrap_or_else(|| "Batch completed with no summary available.".to_string());

        if !file_report.is_empty() {
            synthesis_summary.push_str("\n---\n### 📂 Detected File Changes (Worktrees)\n");
            synthesis_summary.push_str(&file_report.join("\n\n"));
        }

        self.persist_parallel_batch_snapshot(batch_id);
        if let Some(parent_task_id) = self.current_parent_task_id() {
            if let Some(db_handle) = &self.db {
                let db = db_handle.lock();
                let task_store = crate::store::task::TaskStore::from_connection(db.connection());
                if let Ok(Some(parent)) = task_store.get(&parent_task_id) {
                    let risk_flags = batch
                        .children
                        .iter()
                        .filter_map(|child| child.evaluation.as_ref())
                        .flat_map(|evaluation| evaluation.notes.clone())
                        .take(6)
                        .collect::<Vec<_>>();
                    let merge_ready = Self::batch_merge_ready(&batch);
                    let review_summary = serde_json::json!({
                        "changed_files_count": self.git_changes.len(),
                        "ownership_count": batch.children.iter().filter(|child| child.ownership.is_some()).count(),
                        "completed_children": batch.children.iter().filter(|child| matches!(child.status, ParallelChildStatus::Completed)).count(),
                        "failed_children": batch.children.iter().filter(|child| matches!(child.status, ParallelChildStatus::Failed | ParallelChildStatus::Cancelled)).count(),
                        "merge_ready": merge_ready,
                        "risk_flags": risk_flags,
                    });
                    let merged = Self::merge_task_metadata(
                        &parent.metadata,
                        serde_json::json!({
                            "review_summary": review_summary
                        }),
                    );
                    let _ = task_store.update(
                        &parent_task_id,
                        crate::store::task::TaskUpdate {
                            metadata: Some(merged),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        let is_inline_mode = self.agents.parallel_agents_enabled;
        let summary = Self::build_hybrid_synthesis(&batch);

        // Send the report back to the blocking tool call if it exists
        if let Ok(mut tx_guard) = batch.response_tx.lock() {
            if let Some(tx) = tx_guard.take() {
                let _ = tx.send(summary.clone());
            }
        }

        if is_inline_mode {
            if let Some(parent_id) = batch.parent_session_id {
                if let Some(active_loop) = &self.agents.agent_loop {
                    let active_loop = active_loop.clone();
                    let event_tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        let active_session_id = {
                            let config = active_loop.config.read().await;
                            config.session_id.clone()
                        };
                        if active_session_id == parent_id {
                            active_loop
                                .add_user_message("[SYSTEM] All parallel agents have completed. Please provide a compiled synthesis of the results.")
                                .await;
                            if let Some(tx) = event_tx {
                                let _ = tx.send(crate::event::Event::RunSynthesis).await;
                            }
                        }
                    });
                }
            }
        } else {
            self.add_system_message(&summary);
            // Non-inline mode: the response_tx already unblocked the tool call.
            // The parent agent loop will naturally process the tool result and
            // continue. No need for RunSynthesis — it would cause a duplicate
            // run that leaves thinking state stuck.
        }
    }
}
