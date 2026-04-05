//! Task View Refresh Logic
//!
//! Handles refreshing task views for board, list, and inspector modes,
//! including graph summary generation for parallel batches.

use anyhow::Result;

use crate::app::state::AppMode;
use crate::app::App;
use crate::pipeline::UnifiedTrustData;
use crate::ui::widgets::board::KanbanTask;

impl App {
    pub fn refresh_task_views(&mut self) -> Result<()> {
        let db_handle = match &self.db {
            Some(db) => db,
            None => {
                self.task_view_tasks.clear();
                self.task_view_records.clear();
                self.board.load_from_store(Vec::new());
                self.list_selected_task = 0;
                return Ok(());
            }
        };

        let db = db_handle.lock();
        let task_store = crate::store::task::TaskStore::from_connection(db.connection());
        let tasks = task_store.list(crate::store::task::TaskListOptions {
            limit: Some(200),
            ..Default::default()
        })?;

        let task_rows: Vec<KanbanTask> = tasks
            .iter()
            .map(|task| {
                let trust = if let Ok(metadata) =
                    serde_json::from_str::<serde_json::Value>(&task.metadata)
                {
                    UnifiedTrustData::from_metadata(&metadata)
                } else {
                    UnifiedTrustData::from_metadata(&serde_json::json!({}))
                };

                KanbanTask {
                    id: task.id.clone(),
                    title: task.title.clone(),
                    state: task.state,
                    execution_mode: Some(task.execution_mode),
                    priority: task.priority,
                    merge_ready: if trust.merge_readiness.is_some()
                        || trust.review_summary.is_some()
                    {
                        Some(trust.is_merge_ready())
                    } else {
                        None
                    },
                    blocking_count: trust.blocking_count(),
                    qa_iteration: trust.qa_iteration(),
                }
            })
            .collect();
        let graph_summary = if let Some(batch) = self
            .agents
            .parallel_batches
            .values()
            .max_by_key(|batch| batch.started_at)
        {
            let mut lines = vec![format!(
                "#{} {}",
                &batch.id[..batch.id.len().min(8)],
                if batch.select_best { "(best-of-N)" } else { "" }
            )];
            for child in &batch.children {
                let marker = if batch.selected_child_key.as_deref() == Some(child.key.as_str()) {
                    " [winner]"
                } else {
                    ""
                };
                lines.push(format!("{} {}{}", child.key, child.description, marker));
                if !child.depends_on.is_empty() {
                    lines.push(format!("  -> depends on {}", child.depends_on.join(", ")));
                }
            }
            lines
        } else {
            let selected_task_graph = match self.ui.mode {
                AppMode::List => self
                    .task_view_records
                    .get(self.list_selected_task)
                    .map(|task| {
                        task.batch_id
                            .as_deref()
                            .map(|batch_id| {
                                self.graph_summary_from_task_store_batch(&tasks, batch_id)
                            })
                            .filter(|lines| lines.len() > 1)
                            .unwrap_or_else(|| {
                                self.graph_summary_from_task_metadata(&task.metadata)
                            })
                    }),
                AppMode::Board => self
                    .board
                    .selected_task()
                    .and_then(|selected| {
                        self.task_view_records.iter().find(|t| t.id == selected.id)
                    })
                    .map(|task| {
                        task.batch_id
                            .as_deref()
                            .map(|batch_id| {
                                self.graph_summary_from_task_store_batch(&tasks, batch_id)
                            })
                            .filter(|lines| lines.len() > 1)
                            .unwrap_or_else(|| {
                                self.graph_summary_from_task_metadata(&task.metadata)
                            })
                    }),
                _ => None,
            };
            selected_task_graph.unwrap_or_default()
        };

        self.task_view_records = tasks;
        self.task_view_tasks = task_rows.clone();
        self.board.load_from_store(task_rows);
        self.board.set_graph_summary(graph_summary);

        if self.list_selected_task >= self.task_view_tasks.len() {
            self.list_selected_task = self.task_view_tasks.len().saturating_sub(1);
        }

        Ok(())
    }

    pub fn selected_task_record(&self) -> Option<&crate::store::task::Task> {
        match self.ui.mode {
            AppMode::List => self.task_view_records.get(self.list_selected_task),
            AppMode::Board => {
                let selected_id = self.board.selected_task().map(|task| task.id.clone())?;
                self.task_view_records
                    .iter()
                    .find(|task| task.id == selected_id)
            }
            _ => None,
        }
    }
}
