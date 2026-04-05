//! Batch Helper Utilities
//!
//! Small utility methods and types for parallel batch management.

use crate::agent::specialists::AgentType;
use crate::app::{
    App, CandidateEvaluation, ParallelBatchState, ParallelChildStatus, ParallelChildTask,
};

/// Intermediate deliverable extracted from a child task result.
pub(super) struct ChildDeliverable {
    pub status: String,
    pub files_changed: Vec<String>,
    pub decisions: Vec<String>,
    pub code_blocks: Vec<String>,
    pub issues: Vec<String>,
    pub narrative: String,
}

impl App {
    pub(super) fn current_parent_task_id(&self) -> Option<String> {
        self.workspaces
            .get(self.workspace_selected_index)
            .and_then(|workspace| {
                (workspace.workspace_type == crate::app::WorkspaceType::Satellite)
                    .then(|| workspace.id.clone())
            })
    }

    pub(super) fn map_agent_type_to_store_role(
        agent_type: AgentType,
    ) -> crate::store::task::AgentRole {
        match agent_type {
            AgentType::Documentation => crate::store::task::AgentRole::Documenter,
            AgentType::Review | AgentType::Security | AgentType::Testing => {
                crate::store::task::AgentRole::QualityReviewer
            }
            AgentType::General => crate::store::task::AgentRole::Executor,
            _ => crate::store::task::AgentRole::Coder,
        }
    }

    pub(super) fn merge_task_metadata(
        existing: &str,
        patch: serde_json::Value,
    ) -> serde_json::Value {
        fn merge_into(base: &mut serde_json::Value, patch: serde_json::Value) {
            match (base, patch) {
                (serde_json::Value::Object(base_map), serde_json::Value::Object(patch_map)) => {
                    for (key, value) in patch_map {
                        merge_into(
                            base_map.entry(key).or_insert(serde_json::Value::Null),
                            value,
                        );
                    }
                }
                (base_slot, value) => *base_slot = value,
            }
        }

        let mut value = serde_json::from_str(existing).unwrap_or_else(|_| serde_json::json!({}));
        merge_into(&mut value, patch);
        value
    }

    pub(super) fn evaluate_parallel_child(child: &ParallelChildTask) -> CandidateEvaluation {
        let result = child.result.as_deref().unwrap_or("").to_lowercase();
        let mut evaluation = CandidateEvaluation::default();

        evaluation.changed_file_quality += if child.ownership.is_some() { 3 } else { 1 };
        if result.contains("```") || result.contains("diff") || result.contains("patch") {
            evaluation.changed_file_quality += 3;
            evaluation
                .notes
                .push("Output references concrete code or patch details.".to_string());
        }

        if result.contains("test") || result.contains("tests") || result.contains("lint") {
            evaluation.test_lint_outcome += 3;
        }
        if result.contains("pass") || result.contains("passed") || result.contains("green") {
            evaluation.test_lint_outcome += 2;
            evaluation
                .notes
                .push("Candidate reports successful validation signals.".to_string());
        }
        if result.contains("fail") || result.contains("error") {
            evaluation.test_lint_outcome -= 2;
            evaluation
                .notes
                .push("Candidate mentions failing validation or runtime errors.".to_string());
        }

        if child.specialist_role.to_ascii_lowercase().contains("doc")
            || result.contains("readme")
            || result.contains("docs")
            || result.contains("documentation")
        {
            evaluation.docs_completeness += 4;
        }

        evaluation.conflict_risk += 5;
        for marker in [
            "conflict",
            "blocker",
            "uncertain",
            "todo",
            "follow-up",
            "manual",
        ] {
            if result.contains(marker) {
                evaluation.conflict_risk -= 1;
            }
        }
        evaluation.conflict_risk = evaluation.conflict_risk.max(0);

        evaluation.scope_adherence += if child.ownership.is_some() { 3 } else { 1 };
        if let Some(ownership) = &child.ownership {
            let ownership_hits = ownership
                .split(',')
                .filter(|segment| result.contains(segment.trim().to_lowercase().as_str()))
                .count() as i32;
            evaluation.scope_adherence += ownership_hits.min(3);
        }

        if !child.depends_on.is_empty() {
            evaluation.notes.push(format!(
                "Task carries {} dependency edge(s).",
                child.depends_on.len()
            ));
        }

        evaluation.total_score = evaluation.changed_file_quality
            + evaluation.test_lint_outcome
            + evaluation.docs_completeness
            + evaluation.conflict_risk
            + evaluation.scope_adherence;

        if evaluation.notes.is_empty() {
            evaluation
                .notes
                .push("Candidate completed without extra validation detail.".to_string());
        }

        evaluation
    }
    pub(super) fn persist_parallel_batch_snapshot(&self, batch_id: &str) {
        let Some(db_handle) = &self.db else {
            return;
        };
        let Some(batch) = self.agents.parallel_batches.get(batch_id) else {
            return;
        };

        let db = db_handle.lock();
        let task_store = crate::store::task::TaskStore::from_connection(db.connection());
        let batch_graph = serde_json::json!({
            "id": batch.id,
            "reasoning": batch.reasoning,
            "select_best": batch.select_best,
            "selection_criteria": batch.selection_criteria,
            "selected_child_key": batch.selected_child_key,
            "selection_reasoning": batch.selection_reasoning,
            "coordination": {
                "messages": batch.coordination.messages,
                "synthesis_inputs": batch.coordination.synthesis_inputs,
                "unresolved_blockers": batch.coordination.unresolved_blockers,
                "last_progress_update": batch.coordination.last_progress_update,
            },
            "children": batch.children.iter().map(|child| serde_json::json!({
                "key": child.key,
                "description": child.description,
                "task": child.task,
                "agent_type": child.agent_type,
                "specialist_role": child.specialist_role,
                "depends_on": child.depends_on,
                "ownership": child.ownership,
                "task_id": child.task_id,
                "agent_id": child.agent_id,
                "status": format!("{:?}", child.status),
                "result": child.result,
                "evaluation": child.evaluation,
                "progress": child.progress,
                "blocked": child.blocked,
                "blocker_reason": child.blocker_reason,
            })).collect::<Vec<_>>(),
        });

        for child in &batch.children {
            let Some(task_id) = &child.task_id else {
                continue;
            };
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
                        "batch_id": batch.id,
                        "key": child.key,
                        "specialist_role": child.specialist_role,
                        "ownership": child.ownership,
                        "depends_on": child.depends_on,
                        "evaluation": child.evaluation,
                    }
                }),
            );
            let _ = task_store.update(
                task_id,
                crate::store::task::TaskUpdate {
                    metadata: Some(merged),
                    ..Default::default()
                },
            );
        }

        if let Some(parent_task_id) = self.current_parent_task_id() {
            if let Ok(Some(parent)) = task_store.get(&parent_task_id) {
                let merged = Self::merge_task_metadata(
                    &parent.metadata,
                    serde_json::json!({
                        "orchestration_graph": batch_graph
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

    pub(super) fn current_parent_session_id(&self) -> Option<String> {
        self.agents.agent_loop.as_ref().and_then(|agent| {
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let config = agent.config.read().await;
                    Some(config.session_id.clone())
                })
            })
        })
    }

    pub(super) fn synthesize_parallel_batch(batch: &ParallelBatchState) -> Option<String> {
        let mut synthesis = vec![format!(
            "### 📋 Compiled Parallel Execution Report (Batch: `{}`)\n",
            batch.id
        )];

        if !batch.reasoning.is_empty() {
            synthesis.push(format!("**Objective:** {}\n", batch.reasoning));
        }

        let winner_key = batch.selected_child_key.as_ref();
        for child in &batch.children {
            let is_winner = winner_key.map(|k| k == &child.key).unwrap_or(false);
            let status_emoji = match child.status {
                ParallelChildStatus::Completed => "✅",
                ParallelChildStatus::Failed => "❌",
                ParallelChildStatus::Cancelled => "⏭️",
                _ => "⏳",
            };

            synthesis.push(format!(
                "#### {} {} ({}){}:",
                status_emoji,
                child.key,
                child.specialist_role,
                if is_winner { " 🏆 [WINNER]" } else { "" }
            ));

            if !child.task.is_empty() {
                synthesis.push(format!("*Task:* {}", child.task));
            }

            if let Some(result) = child.result.as_deref() {
                let trimmed = result.trim();
                if !trimmed.is_empty() {
                    synthesis.push(format!("*Summary Output:* {}\n", trimmed));
                }
            }

            if let Some(evaluation) = &child.evaluation {
                if !evaluation.notes.is_empty() {
                    synthesis.push(format!("*Evaluation:* {}", evaluation.notes.join("; ")));
                }
                synthesis.push(format!(
                    "*Score:* **{}** (Files: {}, Tests: {}, Docs: {}, Scope: {})\n",
                    evaluation.total_score,
                    evaluation.changed_file_quality,
                    evaluation.test_lint_outcome,
                    evaluation.docs_completeness,
                    evaluation.scope_adherence
                ));
            }
        }

        if let Some(reasoning) = &batch.selection_reasoning {
            synthesis.push(format!("---\n**Selection Reasoning:**\n{}\n", reasoning));
        }

        Some(synthesis.join("\n"))
    }
}
