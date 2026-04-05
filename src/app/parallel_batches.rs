//! Parallel Batch State Restoration and Graph Summaries
//!
//! Handles serializing/deserializing parallel batch state from task metadata
//! and generating human-readable graph summaries for the UI.

use std::time::Instant;

use crate::app::handlers::agent::coordination::{
    BatchCoordination, CoordinationMessage, SynthesisInput, UnresolvedBlocker,
};
use crate::app::state::{CandidateEvaluation, ParallelBatchState, ParallelChildStatus};
use crate::app::App;

impl App {
    pub fn restore_parallel_batches_from_metadata(&mut self, metadata: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata) else {
            return;
        };
        let Some(batches) = value
            .get("parallel_batches")
            .and_then(|batches| batches.as_array())
        else {
            return;
        };

        self.agents.parallel_batches.clear();
        for batch in batches {
            let Some(id) = batch.get("id").and_then(|v| v.as_str()) else {
                continue;
            };

            let coordination = batch
                .get("coordination")
                .map(|coord| {
                    let messages: Vec<CoordinationMessage> = coord
                        .get("messages")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| serde_json::from_value(m.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();

                    let synthesis_inputs: Vec<SynthesisInput> = coord
                        .get("synthesis_inputs")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|s| serde_json::from_value(s.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();

                    let unresolved_blockers: Vec<UnresolvedBlocker> = coord
                        .get("unresolved_blockers")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|b| serde_json::from_value(b.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();

                    let last_progress_update = coord
                        .get("last_progress_update")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);

                    BatchCoordination {
                        messages,
                        synthesis_inputs,
                        unresolved_blockers,
                        last_progress_update,
                    }
                })
                .unwrap_or_default();

            let children = batch
                .get("children")
                .and_then(|v| v.as_array())
                .map(|children| {
                    children
                        .iter()
                        .filter_map(|child| {
                            Some(crate::app::ParallelChildTask {
                                key: child.get("key")?.as_str()?.to_string(),
                                description: child.get("description")?.as_str()?.to_string(),
                                task: child.get("task")?.as_str()?.to_string(),
                                agent_type: child.get("agent_type")?.as_str()?.to_string(),
                                specialist_role: child
                                    .get("specialist_role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Executor")
                                    .to_string(),
                                depends_on: child
                                    .get("depends_on")
                                    .and_then(|v| v.as_array())
                                    .map(|deps| {
                                        deps.iter()
                                            .filter_map(|dep| dep.as_str().map(str::to_string))
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default(),
                                ownership: child
                                    .get("ownership")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                task_id: child
                                    .get("task_id")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                agent_id: child
                                    .get("agent_id")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                status: match child
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Pending")
                                {
                                    "Running" => ParallelChildStatus::Running,
                                    "Completed" => ParallelChildStatus::Completed,
                                    "Failed" => ParallelChildStatus::Failed,
                                    "Cancelled" => ParallelChildStatus::Cancelled,
                                    _ => ParallelChildStatus::Pending,
                                },
                                result: child
                                    .get("result")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                evaluation: child.get("evaluation").cloned().and_then(|value| {
                                    serde_json::from_value::<CandidateEvaluation>(value).ok()
                                }),
                                progress: child
                                    .get("progress")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u8,
                                blocked: child
                                    .get("blocked")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                blocker_reason: child
                                    .get("blocker_reason")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                messages_sent: child
                                    .get("messages_sent")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as usize,
                                messages_received: child
                                    .get("messages_received")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as usize,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            self.agents.parallel_batches.insert(
                id.to_string(),
                ParallelBatchState {
                    id: id.to_string(),
                    parent_session_id: batch
                        .get("parent_session_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    reasoning: batch
                        .get("reasoning")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    select_best: batch
                        .get("select_best")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    selection_criteria: batch
                        .get("selection_criteria")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    selected_child_key: batch
                        .get("selected_child_key")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    selection_reasoning: batch
                        .get("selection_reasoning")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    started_at: Instant::now(),
                    completed_at: Some(Instant::now()),
                    children,
                    coordination,
                    response_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
                },
            );
        }
    }

    pub fn graph_summary_from_task_metadata(&self, metadata: &str) -> Vec<String> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata) else {
            return Vec::new();
        };
        let Some(orchestration) = value.get("orchestration") else {
            return Vec::new();
        };
        let Some(batches) = orchestration
            .get("parallel_batches")
            .and_then(|v| v.as_array())
        else {
            return Vec::new();
        };
        let Some(batch) = batches.last() else {
            return Vec::new();
        };
        let Some(id) = batch.get("id").and_then(|v| v.as_str()) else {
            return Vec::new();
        };
        let selected_child_key = batch
            .get("selected_child_key")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let mut lines = vec![format!("#{}", &id[..id.len().min(8)])];
        if let Some(children) = batch.get("children").and_then(|v| v.as_array()) {
            for child in children {
                if let Some(key) = child.get("key").and_then(|v| v.as_str()) {
                    let description = child
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let marker = if selected_child_key.as_deref() == Some(key) {
                        " [winner]"
                    } else {
                        ""
                    };
                    lines.push(format!("{} {}{}", key, description, marker));
                }
            }
        }
        lines
    }

    pub fn graph_summary_from_task_store_batch(
        &self,
        tasks: &[crate::store::task::Task],
        batch_id: &str,
    ) -> Vec<String> {
        let mut lines = vec![format!("#{}", &batch_id[..batch_id.len().min(8)])];
        let mut batch_tasks = tasks
            .iter()
            .filter(|task| task.batch_id.as_deref() == Some(batch_id))
            .collect::<Vec<_>>();
        batch_tasks.sort_by_key(|task| task.created_at.clone());

        for task in batch_tasks {
            let metadata =
                serde_json::from_str::<serde_json::Value>(&task.metadata).unwrap_or_default();
            let specialist_role = metadata
                .get("orchestration_node")
                .and_then(|node| node.get("specialist_role"))
                .and_then(|v| v.as_str())
                .unwrap_or("Executor");
            lines.push(format!("{} {} [{}]", task.id, task.title, specialist_role));
            if let Some(depends_on) = metadata
                .get("orchestration_node")
                .and_then(|node| node.get("depends_on"))
                .and_then(|v| v.as_array())
            {
                let deps = depends_on
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>();
                if !deps.is_empty() {
                    lines.push(format!("  -> depends on {}", deps.join(", ")));
                }
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{
        AgentState, CandidateEvaluation, LayoutState, ParallelBatchState, ParallelChildStatus,
        ParallelChildTask, RightPaneTab, SessionState, ToolState, UIState,
    };
    use crate::app::App;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_app() -> App {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let tool_coordinator = Arc::new(crate::agent::ToolCoordinator::new());
            let mcp_manager = Arc::new(crate::mcp::McpManager::new());
            let config =
                crate::config::load_config(crate::config::LoadConfigOptions::default()).unwrap();
            let registry = Arc::new(tokio::sync::RwLock::new(
                crate::providers::ModelRegistry::new(),
            ));
            let symbols = crate::services::SymbolExtractor::new();
            let board = crate::ui::widgets::board::Board::new();
            let orch_config = crate::pipeline::orchestrator::OrchestratorConfig::default();
            let orchestrator = Arc::new(
                crate::pipeline::PipelineOrchestrator::new(orch_config, None)
                    .await
                    .unwrap(),
            );
            let subagents = Arc::new(crate::agent::SubAgentManager::new());

            App {
                cwd: Some("/tmp/test".to_string()),
                base_cwd: Some("/tmp/test".to_string()),
                model: Some("test-model".to_string()),
                config,
                subagents,
                provider: None,
                db: None,
                ui: UIState::default(),
                session: SessionState::default(),
                agents: AgentState::default(),
                workspaces: Vec::new(),
                workspace_selected_index: 0,
                git_changes: Vec::new(),
                active_branch: "main".to_string(),
                pr_number: None,
                notifications: Vec::new(),
                autonomous_mode: false,
                autonomous_iterations: 0,
                tools: ToolState::new(tool_coordinator),
                animation_frame: 0,
                last_update: Instant::now(),
                registry,
                last_git_refresh: Instant::now(),
                last_workspace_refresh: Instant::now(),
                last_orchestrator_refresh: Instant::now(),
                mcp_manager,
                ipc_client: None,
                ipc_handle: None,
                event_tx: None,
                selected_right_pane_tab: RightPaneTab::Agent,
                should_quit: false,
                ctrl_c_count: 0,
                last_ctrl_c_time: None,
                command_palette_filter: String::new(),
                command_palette_selected: 0,
                diff_view: None,
                diff_preview: None,
                selected_diff_index: 0,
                undo_picker: None,
                session_picker: None,
                symbols,
                memory_search: None,
                permission_manager: None,
                board,
                orchestrator,
                layout: LayoutState::default(),
                workspace_states: HashMap::new(),
                background_active_tasks: Vec::new(),
                background_queue_stats: crate::pipeline::QueueStats::default(),
                background_worker_stats: crate::pipeline::WorkerPoolStats::default(),
                task_view_tasks: Vec::new(),
                task_view_records: Vec::new(),
                list_selected_task: 0,
            }
        })
    }

    fn make_test_batch() -> ParallelBatchState {
        ParallelBatchState {
            id: "batch-test-123".to_string(),
            parent_session_id: Some("session-abc".to_string()),
            reasoning: "Test orchestration".to_string(),
            select_best: true,
            selection_criteria: Some("Best code quality".to_string()),
            selected_child_key: Some("child-2".to_string()),
            selection_reasoning: Some("Higher test coverage".to_string()),
            started_at: Instant::now(),
            completed_at: Some(Instant::now()),
            children: vec![
                ParallelChildTask {
                    key: "child-1".to_string(),
                    description: "Implement feature A".to_string(),
                    task: "Create the login feature".to_string(),
                    agent_type: "general".to_string(),
                    specialist_role: "Coder".to_string(),
                    depends_on: vec![],
                    ownership: Some("src/auth/*".to_string()),
                    task_id: Some("task-1".to_string()),
                    agent_id: Some("agent-1".to_string()),
                    status: ParallelChildStatus::Completed,
                    result: Some("Implemented login".to_string()),
                    evaluation: Some(CandidateEvaluation {
                        changed_file_quality: 3,
                        test_lint_outcome: 4,
                        docs_completeness: 2,
                        conflict_risk: 1,
                        scope_adherence: 3,
                        total_score: 13,
                        notes: vec!["Good coverage".to_string()],
                    }),
                    progress: 100,
                    blocked: false,
                    blocker_reason: None,
                    messages_sent: 5,
                    messages_received: 3,
                },
                ParallelChildTask {
                    key: "child-2".to_string(),
                    description: "Implement feature B".to_string(),
                    task: "Create the dashboard".to_string(),
                    agent_type: "general".to_string(),
                    specialist_role: "Coder".to_string(),
                    depends_on: vec!["child-1".to_string()],
                    ownership: Some("src/dashboard/*".to_string()),
                    task_id: Some("task-2".to_string()),
                    agent_id: Some("agent-2".to_string()),
                    status: ParallelChildStatus::Completed,
                    result: Some("Implemented dashboard".to_string()),
                    evaluation: Some(CandidateEvaluation {
                        changed_file_quality: 5,
                        test_lint_outcome: 5,
                        docs_completeness: 4,
                        conflict_risk: 0,
                        scope_adherence: 5,
                        total_score: 19,
                        notes: vec!["Excellent".to_string()],
                    }),
                    progress: 100,
                    blocked: false,
                    blocker_reason: None,
                    messages_sent: 8,
                    messages_received: 6,
                },
                ParallelChildTask {
                    key: "child-3".to_string(),
                    description: "Implement feature C".to_string(),
                    task: "Create the reports".to_string(),
                    agent_type: "general".to_string(),
                    specialist_role: "Coder".to_string(),
                    depends_on: vec!["child-1".to_string()],
                    ownership: Some("src/reports/*".to_string()),
                    task_id: Some("task-3".to_string()),
                    agent_id: Some("agent-3".to_string()),
                    status: ParallelChildStatus::Failed,
                    result: Some("Timed out".to_string()),
                    evaluation: Some(CandidateEvaluation {
                        changed_file_quality: 1,
                        test_lint_outcome: 1,
                        docs_completeness: 1,
                        conflict_risk: 5,
                        scope_adherence: 2,
                        total_score: 10,
                        notes: vec!["Timeout".to_string()],
                    }),
                    progress: 45,
                    blocked: false,
                    blocker_reason: None,
                    messages_sent: 2,
                    messages_received: 1,
                },
            ],
            coordination: BatchCoordination::new(),
            response_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    #[test]
    fn test_persist_restore_roundtrip() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        assert_eq!(app.agents.parallel_batches.len(), 1);
        let restored = app.agents.parallel_batches.get("batch-test-123").unwrap();
        assert_eq!(restored.id, "batch-test-123");
        assert_eq!(restored.reasoning, "Test orchestration");
        assert!(restored.select_best);
        assert_eq!(
            restored.selection_criteria.as_deref(),
            Some("Best code quality")
        );
    }

    #[test]
    fn test_children_survive_restore() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        let restored = app.agents.parallel_batches.get("batch-test-123").unwrap();
        assert_eq!(restored.children.len(), 3);

        let child1 = restored
            .children
            .iter()
            .find(|c| c.key == "child-1")
            .unwrap();
        assert_eq!(child1.description, "Implement feature A");
        assert_eq!(child1.status, ParallelChildStatus::Completed);
        assert_eq!(child1.progress, 100);
    }

    #[test]
    fn test_dependency_edges_survive_restore() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        let restored = app.agents.parallel_batches.get("batch-test-123").unwrap();

        let child2 = restored
            .children
            .iter()
            .find(|c| c.key == "child-2")
            .unwrap();
        assert_eq!(child2.depends_on, vec!["child-1".to_string()]);

        let child3 = restored
            .children
            .iter()
            .find(|c| c.key == "child-3")
            .unwrap();
        assert_eq!(child3.depends_on, vec!["child-1".to_string()]);

        let child1 = restored
            .children
            .iter()
            .find(|c| c.key == "child-1")
            .unwrap();
        assert!(child1.depends_on.is_empty());
    }

    #[test]
    fn test_winner_selection_survives_restore() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        let restored = app.agents.parallel_batches.get("batch-test-123").unwrap();
        assert_eq!(restored.selected_child_key.as_deref(), Some("child-2"));
        assert_eq!(
            restored.selection_reasoning.as_deref(),
            Some("Higher test coverage")
        );
    }

    #[test]
    fn test_evaluation_survives_restore() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        let restored = app.agents.parallel_batches.get("batch-test-123").unwrap();
        let child2 = restored
            .children
            .iter()
            .find(|c| c.key == "child-2")
            .unwrap();

        let eval = child2.evaluation.as_ref().unwrap();
        assert_eq!(eval.total_score, 19);
        assert!(eval.notes.contains(&"Excellent".to_string()));
    }

    #[test]
    fn test_graph_summary_shows_winner() {
        let batch = make_test_batch();
        let metadata = serde_json::json!({
            "parallel_batches": [serde_json::to_value(&batch).unwrap()]
        });
        let metadata_str = metadata.to_string();

        let app = test_app();
        let lines = app.graph_summary_from_task_metadata(&metadata_str);

        assert!(!lines.is_empty());
        assert!(lines
            .iter()
            .any(|l| l.contains("[winner]") || l.contains("child-2")));
    }

    #[test]
    fn test_multiple_batches_restore() {
        let mut batch1 = make_test_batch();
        batch1.id = "batch-1".to_string();
        batch1.selected_child_key = Some("child-1".to_string());

        let mut batch2 = make_test_batch();
        batch2.id = "batch-2".to_string();
        batch2.selected_child_key = Some("child-2".to_string());

        let metadata = serde_json::json!({
            "parallel_batches": [
                serde_json::to_value(&batch1).unwrap(),
                serde_json::to_value(&batch2).unwrap(),
            ]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        assert_eq!(app.agents.parallel_batches.len(), 2);
        assert!(app.agents.parallel_batches.contains_key("batch-1"));
        assert!(app.agents.parallel_batches.contains_key("batch-2"));
    }

    #[test]
    fn test_empty_metadata_handled() {
        let mut app = test_app();
        app.agents
            .parallel_batches
            .insert("old-batch".to_string(), make_test_batch());

        app.restore_parallel_batches_from_metadata("{}");
        assert!(app.agents.parallel_batches.is_empty());

        app.restore_parallel_batches_from_metadata("not valid json");
        assert!(app.agents.parallel_batches.is_empty());
    }

    #[test]
    fn test_partial_child_data_handled() {
        let metadata = serde_json::json!({
            "parallel_batches": [{
                "id": "batch-partial",
                "children": [{
                    "key": "incomplete-child"
                }]
            }]
        });
        let metadata_str = metadata.to_string();

        let mut app = test_app();
        app.restore_parallel_batches_from_metadata(&metadata_str);

        assert_eq!(app.agents.parallel_batches.len(), 1);
        let restored = app.agents.parallel_batches.get("batch-partial").unwrap();
        assert!(restored.children.is_empty());
    }
}
