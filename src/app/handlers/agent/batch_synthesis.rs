//! Batch Synthesis and Deliverable Formatting
//!
//! Builds hybrid synthesis reports from parallel batch results and
//! formats child deliverables for display and LLM consumption.

use super::batch_helpers::ChildDeliverable;
use super::extraction::status_icon;
use crate::app::{App, ParallelBatchState, ParallelChildStatus, ParallelChildTask};

impl App {
    const MAX_CODE_BLOCKS_TOKENS: usize = 2500;
    const MAX_NARRATIVE_TOKENS: usize = 1500;

    pub(super) fn build_hybrid_synthesis(batch: &ParallelBatchState) -> String {
        let mut synthesis = vec!["## Parallel Execution Synthesis\n".to_string()];

        if !batch.reasoning.is_empty() {
            synthesis.push(format!("**Objective:** {}\n", batch.reasoning));
        }

        let winner_key = batch.selected_child_key.as_ref();
        let completed_count = batch
            .children
            .iter()
            .filter(|c| matches!(c.status, ParallelChildStatus::Completed))
            .count();

        synthesis.push(format!(
            "**Completed:** {}/{}\n",
            completed_count,
            batch.children.len()
        ));

        if let Some(reasoning) = &batch.selection_reasoning {
            synthesis.push(format!(
                "**Selection:** {} — {}\n",
                winner_key.as_ref().map_or("N/A", |k| k.as_str()),
                reasoning
            ));
        }

        synthesis.push("\n---\n".to_string());

        let max_full_context = if batch.select_best { 1 } else { 2 };

        for (i, child) in batch.children.iter().enumerate() {
            let is_winner = winner_key
                .as_ref()
                .map(|k| k.as_str() == child.key.as_str())
                .unwrap_or(false);
            let include_full = (is_winner && batch.select_best)
                || (i < max_full_context && child.status == ParallelChildStatus::Completed);

            let deliverable = Self::extract_deliverable(child, include_full);
            synthesis.push(Self::format_deliverable(
                &child.key,
                &child.specialist_role,
                &child.task,
                is_winner,
                deliverable,
            ));
            synthesis.push("\n---\n".to_string());
        }

        synthesis.push("*Synthesize the results above into a cohesive response.*".to_string());

        synthesis.join("")
    }

    pub(super) fn extract_deliverable(
        child: &ParallelChildTask,
        include_full: bool,
    ) -> ChildDeliverable {
        let result = child.result.as_deref().unwrap_or("");

        let status = match child.status {
            ParallelChildStatus::Completed => "Completed",
            ParallelChildStatus::Failed => "Failed",
            ParallelChildStatus::Cancelled => "Cancelled",
            ParallelChildStatus::Pending => "Pending",
            ParallelChildStatus::Running => "Running",
        }
        .to_string();

        let files_changed = Self::extract_files_changed(result);
        let decisions = Self::extract_decisions(result);
        let issues = Self::extract_issues(result);

        let (code_blocks, narrative) = if include_full {
            let blocks = Self::extract_code_blocks(result, Self::MAX_CODE_BLOCKS_TOKENS);
            let narrative_text = Self::extract_narrative(result, Self::MAX_NARRATIVE_TOKENS);
            (blocks, narrative_text)
        } else {
            let blocks = Self::extract_code_blocks(result, 800);
            let narrative_text = Self::extract_narrative(result, 500);
            (blocks, narrative_text)
        };

        ChildDeliverable {
            status,
            files_changed,
            decisions,
            code_blocks,
            issues,
            narrative,
        }
    }

    pub(super) fn format_deliverable(
        key: &str,
        role: &str,
        task: &str,
        is_winner: bool,
        deliverable: ChildDeliverable,
    ) -> String {
        let mut output = vec![];

        output.push(format!(
            "### {} [{}] {}{}\n",
            status_icon(&deliverable.status),
            key,
            role,
            if is_winner { " 🏆" } else { "" }
        ));

        output.push(format!("**Task:** {}\n", task));

        if !deliverable.files_changed.is_empty() {
            output.push(format!(
                "**Files Changed:** {}\n",
                deliverable.files_changed.join(", ")
            ));
        }

        if !deliverable.decisions.is_empty() {
            output.push("**Decisions:**\n".to_string());
            for decision in &deliverable.decisions {
                output.push(format!("- {}\n", decision));
            }
        }

        if !deliverable.code_blocks.is_empty() {
            output.push("**Code Blocks:**\n".to_string());
            for block in &deliverable.code_blocks {
                output.push(format!("```\n{}\n```\n", block));
            }
        }

        if !deliverable.issues.is_empty() {
            output.push(format!("**Issues:** {}\n", deliverable.issues.join("; ")));
        }

        output.push(format!("**Summary:** {}\n", deliverable.narrative));

        output.join("")
    }
}
