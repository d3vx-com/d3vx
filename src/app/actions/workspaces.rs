use crate::app::actions::MessageExecutionFlags;
use crate::app::state::WorkspaceType;
use crate::app::{App, WorkspaceStatus, WorkspaceTask};
use crate::pipeline::github::{GitHubApiClient, GitHubConfig};
use crate::pipeline::orchestrator::TaskAuthority;
use crate::store::task::{ExecutionMode, NewTask, TaskState, TaskStore, TaskUpdate};
use anyhow::Result;
use tracing::info;

impl App {
    /// Refresh the list of workspaces from the database
    pub fn refresh_workspaces(&mut self) -> Result<()> {
        let current_dir = self.cwd.clone().unwrap_or_else(|| ".".to_string());

        // Always add the "Anchor" (Main Chat) first
        let mut next_workspaces = vec![WorkspaceTask {
            id: "home".to_string(),
            name: "Main Chat".to_string(),
            branch: self.active_branch.clone(),
            path: current_dir.clone(),
            workspace_type: WorkspaceType::Anchor,
            changes_added: 0,
            changes_removed: 0,
            status: WorkspaceStatus::Idle,
            phase: None,
        }];

        let db = match &self.db {
            Some(db) => db.lock(),
            None => {
                self.workspaces = next_workspaces;
                return Ok(());
            }
        };

        let mut stmt = db.prepare(
            "SELECT id, title, worktree_branch, pipeline_phase, worktree_path FROM tasks WHERE state != 'ARCHIVE' ORDER BY updated_at DESC LIMIT 20"
        )?;

        let workspaces = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let branch: String = row.get(2).unwrap_or_else(|_| "main".to_string());
            let phase: Option<String> = row.get(3)?;
            let path: String = row.get(4).unwrap_or_else(|_| current_dir.clone());

            let status = match phase.as_deref() {
                Some("PLAN") | Some("IMPLEMENT") => WorkspaceStatus::Thinking,
                Some("REVIEW") => WorkspaceStatus::ReadyToMerge,
                _ => WorkspaceStatus::Idle,
            };

            Ok(WorkspaceTask {
                id,
                name: title,
                branch,
                path,
                workspace_type: WorkspaceType::Satellite,
                changes_added: 0,
                changes_removed: 0,
                status,
                phase,
            })
        })?;

        let current_path = if current_dir == "." {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else {
            std::path::PathBuf::from(&current_dir)
        };
        let canonical_current = current_path.canonicalize().unwrap_or(current_path);

        for ws in workspaces {
            if let Ok(ws) = ws {
                let ws_path = std::path::Path::new(&ws.path);
                let canonical_ws = ws_path
                    .canonicalize()
                    .unwrap_or_else(|_| ws_path.to_path_buf());

                // Only show if the task's path starts with our current path (or vice versa if it's a parent task?)
                if canonical_ws.starts_with(&canonical_current) {
                    next_workspaces.push(ws);
                }
            }
        }

        // Add active sub-agents
        let agents = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.subagents.list())
        });
        for agent in agents {
            next_workspaces.push(WorkspaceTask {
                id: agent.id.clone(),
                name: agent.task.clone(),
                branch: "sub-agent".to_string(),
                path: current_dir.clone(),
                workspace_type: WorkspaceType::SubAgent,
                changes_added: 0,
                changes_removed: 0,
                status: match agent.status {
                    crate::agent::SubAgentStatus::Running => WorkspaceStatus::Thinking,
                    _ => WorkspaceStatus::Idle,
                },
                phase: Some("Sub-agent".to_string()),
            });
        }

        self.workspaces = next_workspaces;

        Ok(())
    }

    /// Start a new Vex task (isolated worktree/mirror) with a description
    pub fn start_vex_task(&mut self, description: &str) -> Result<()> {
        self.start_vex_task_with_flags(
            description,
            MessageExecutionFlags {
                vex: true,
                ..Default::default()
            },
        )
    }

    /// Start a new Vex task with explicit execution policy flags.
    pub fn start_vex_task_with_flags(
        &mut self,
        description: &str,
        flags: MessageExecutionFlags,
    ) -> Result<()> {
        if description.trim().is_empty() {
            self.add_system_message("Vex task requires a description.");
            return Ok(());
        }

        // Create branch name from description (lowercase, no spaces)
        let branch_name = format!(
            "vex/{}",
            description
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
        );

        // Limit branch name length and remove trailing hyphens
        let mut branch_name = branch_name.trim_matches('-').to_string();
        if branch_name.len() > 30 {
            branch_name.truncate(30);
            branch_name = branch_name.trim_matches('-').to_string();
        }

        self.add_system_message(&format!(
            "🚀 Starting background Vex task: \"{}\"{}{}{}",
            description,
            if flags.review_required() {
                " [review]"
            } else {
                ""
            },
            if flags.merge { " [merge-if-safe]" } else { "" },
            if flags.docs { " [docs]" } else { "" }
        ));

        let project_path = self.cwd.clone().unwrap_or_else(|| ".".to_string());
        let vex_handle = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.orchestrator.create_vex_task(
                description,
                &project_path,
                Some(&branch_name),
            ))
        })?;
        let task_id = vex_handle.task_id.clone();

        // 1. Create task/session records for the TUI workspace model
        {
            let db_handle = self
                .db
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database not available"))?;
            let db = db_handle.lock();
            let task_store = TaskStore::from_connection(db.connection());

            task_store.create(NewTask {
                id: Some(task_id.clone()),
                title: description.to_string(),
                description: None,
                state: Some(TaskState::Plan),
                priority: None,
                batch_id: None,
                max_retries: None,
                depends_on: None,
                metadata: Some(serde_json::json!({
                    "execution_policy": {
                        "review_required": flags.review_required(),
                        "auto_merge_if_safe": flags.merge,
                        "docs_required": flags.docs
                    }
                })),
                project_path: Some(project_path.clone()),
                agent_role: None,
                execution_mode: Some(ExecutionMode::Vex),
                repo_root: self.cwd.clone(),
                task_scope_path: None,
                scope_mode: None,
                parent_task_id: None,
            })?;

            // Update branch name and pipeline phase
            db.execute(
                "UPDATE tasks SET worktree_branch = ?1, pipeline_phase = ?2 WHERE id = ?3",
                rusqlite::params![&branch_name, "PLAN", &task_id],
            )?;

            // Also create an associated session so isolation works immediately
            let session_store =
                crate::store::session::SessionStore::from_connection(db.connection());
            session_store.create(crate::store::session::NewSession {
                id: None, // Auto-generate
                task_id: Some(task_id.clone()),
                provider: "anthropic".to_string(), // Default
                model: self
                    .model
                    .clone()
                    .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string()),
                messages: Some("[]".to_string()),
                token_count: Some(0),
                summary: Some(description.to_string()),
                project_path: Some(project_path.clone()),
                parent_session_id: None,
                metadata: None,
                state: None,
            })?;
        }

        let execution_policy_metadata = serde_json::json!({
            "execution_policy": {
                "review_required": flags.review_required(),
                "auto_merge_if_safe": flags.merge,
                "docs_required": flags.docs
            }
        });
        {
            let orchestrator = self.orchestrator.clone();
            let task_id_clone = task_id.clone();
            let policy_metadata = execution_policy_metadata.clone();
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                let _ = rt.block_on(async {
                    orchestrator
                        .patch_task_metadata(&task_id_clone, policy_metadata)
                        .await
                });
            });
        }

        if let Some(integrations) = &self.config.integrations {
            if let Some(github) = &integrations.github {
                if github.auto_create_issues_for_manual_tasks {
                    if let Some(repository) = &github.repository {
                        let github_config = GitHubConfig {
                            repositories: vec![repository.clone()],
                            trigger_labels: vec!["d3vx".to_string()],
                            auto_process_labels: vec!["d3vx-auto".to_string()],
                            poll_interval_secs: 300,
                            webhook_secret: None,
                            sync_status: true,
                            token_env: github.token_env.clone(),
                            api_base_url: github.api_base_url.clone(),
                        };

                        let issue_body = format!(
                            "Autonomous task created by d3vx.\n\nTask: {}\nProject Path: {}\nExecution Mode: VEX\nBranch: {}\nReview Required: {}\nAuto Merge If Safe: {}\nDocs Required: {}\n",
                            description,
                            project_path,
                            branch_name,
                            flags.review_required(),
                            flags.merge,
                            flags.docs,
                        );

                        let issue_result = tokio::task::block_in_place(|| {
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(async {
                                let client = GitHubApiClient::from_config(&github_config)?;
                                client
                                    .create_issue(
                                        repository,
                                        description,
                                        &issue_body,
                                        vec!["d3vx".to_string(), "vex".to_string()],
                                    )
                                    .await
                            })
                        });

                        match issue_result {
                            Ok(issue) => {
                                {
                                    let metadata = serde_json::json!({
                                        "github": {
                                            "repository": issue.repository,
                                            "issue_number": issue.number,
                                            "issue_title": issue.title
                                        }
                                    });
                                    {
                                        let db_handle = self.db.as_ref().ok_or_else(|| {
                                            anyhow::anyhow!("Database not available")
                                        })?;
                                        let db = db_handle.lock();
                                        let task_store =
                                            TaskStore::from_connection(db.connection());
                                        let _ = task_store.update(
                                            &task_id,
                                            TaskUpdate {
                                                metadata: Some(metadata.clone()),
                                                ..Default::default()
                                            },
                                        );
                                    }
                                    let orchestrator = self.orchestrator.clone();
                                    let task_id_clone = task_id.clone();
                                    tokio::task::block_in_place(|| {
                                        let rt = tokio::runtime::Handle::current();
                                        let _ = rt.block_on(async {
                                            orchestrator
                                                .patch_task_metadata(&task_id_clone, metadata)
                                                .await
                                        });
                                    });
                                }
                                self.add_system_message(&format!(
                                    "Linked task {} to GitHub issue #{} in {}.",
                                    task_id, issue.number, repository
                                ));
                            }
                            Err(error) => {
                                self.add_system_message(&format!(
                                    "GitHub issue auto-creation failed for task {}: {}",
                                    task_id, error
                                ));
                            }
                        }
                    }
                }
            }
        }

        // 2. Create workspace (git worktree or shadow mirror)
        let worktree_path = self.create_task_workspace(&task_id, &branch_name)?;

        // 3. Update DB with path
        {
            let db_handle = self
                .db
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database not available"))?;
            let db = db_handle.lock();
            db.execute(
                "UPDATE tasks SET worktree_path = ? WHERE id = ?",
                (worktree_path.to_string_lossy().to_string(), &task_id),
            )?;
        }

        let workspace_metadata = serde_json::json!({
            "workspace": {
                "project_path": project_path,
                "branch_name": branch_name,
                "worktree_path": worktree_path.to_string_lossy().to_string()
            }
        });
        let orchestrator = self.orchestrator.clone();
        let task_id_clone = task_id.clone();
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let _ = rt.block_on(async {
                orchestrator
                    .patch_task_metadata(&task_id_clone, workspace_metadata)
                    .await
            });
        });

        self.add_system_message(&format!(
            "✅ Background workspace created at: {}",
            worktree_path.display()
        ));

        // 4. Refresh workspaces list
        self.refresh_workspaces()?;

        // 5. Dispatch through the orchestrator-owned queue
        let orchestrator = self.orchestrator.clone();
        tokio::spawn(async move {
            let _ = orchestrator.dispatch_tasks_parallel(3).await;
        });

        self.add_system_message(&format!(
            "Background task {} is running. Open /agents or press Ctrl+A to monitor it. Policy: review={} merge={} docs={}. Use Alt+Left/Right to switch workspaces, and Ctrl+L to show or hide the navigator.",
            task_id,
            flags.review_required(),
            flags.merge,
            flags.docs,
        ));

        Ok(())
    }

    /// Switch the active workspace and load its context
    pub fn switch_workspace(&mut self, index: usize) -> Result<()> {
        if index >= self.workspaces.len() {
            return Ok(());
        }

        if index == self.workspace_selected_index && !self.session.messages.is_empty() {
            return Ok(());
        }

        // 1. Save current state to cache BEFORE switching
        let current_ws_id = self.workspaces[self.workspace_selected_index].id.clone();
        let current_state = crate::app::WorkspaceState {
            messages: std::mem::take(&mut self.session.messages),
            session_id: self.session.session_id.take(),
            streaming_message: std::mem::take(&mut self.agents.streaming_message),
            thinking: self.session.thinking.clone(),
        };
        self.workspace_states
            .insert(current_ws_id.clone(), current_state);

        // 2. Update current workspace index
        self.workspace_selected_index = index;
        let task = self.workspaces[index].clone();
        let new_ws_id = task.id.clone();

        // 3. Try to restore from memory cache (includes ongoing background sessions)
        if let Some(cached) = self.workspace_states.remove(&new_ws_id) {
            self.session.messages = cached.messages;
            self.session.session_id = cached.session_id;
            self.agents.streaming_message = cached.streaming_message;
            self.session.thinking = cached.thinking;

            // Restore agent loop for this workspace
            self.agents.agent_loop = self.agents.workspace_agents.get(&new_ws_id).cloned();

            info!("Restored workspace state from memory: {}", new_ws_id);
            return Ok(());
        }

        // 4. Fallback: Load the session for this workspace from Database
        let db_handle = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not available"))?;
        let db = db_handle.lock();
        let session_store = crate::store::session::SessionStore::from_connection(db.connection());

        // Find session for this task
        let session_id = if task.id == "home" {
            self.session.home_session_id.clone()
        } else if task.workspace_type == WorkspaceType::SubAgent {
            Some(task.id.clone())
        } else {
            let sessions = session_store.list(crate::store::session::SessionListOptions {
                task_id: Some(task.id.clone()),
                ..Default::default()
            })?;
            sessions.first().map(|s| s.id.clone())
        };

        drop(db); // Drop before resume_session which locks again

        if let Some(sid) = session_id {
            // Use block_in_place for async resume
            let sid_clone = sid.clone();
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                let _ = rt.block_on(self.resume_session(&sid_clone));
            });
        } else {
            // Start fresh
            self.session.session_id = None;
            self.session.messages = Vec::new();
            self.add_system_message(&format!("Started fresh context for: {}", task.name));
        }

        // Initialize agent for this workspace if it doesn't exist
        if !self.agents.workspace_agents.contains_key(&new_ws_id) {
            let sid = self.session.session_id.clone();
            let cwd = Some(task.path.clone());
            let model = self.model.clone();
            let tools = self.tools.tool_coordinator.clone();

            let (agent, receiver, _hint) = Self::create_agent(
                &cwd,
                &model,
                &sid,
                tools,
                self.ui.plan_mode,
                self.agents.parallel_agents_enabled,
                self.ui.focus_mode,
                self.permission_manager.clone(),
                self.db.clone(),
            )?;
            if let Some(agent) = agent {
                self.agents
                    .workspace_agents
                    .insert(new_ws_id.clone(), agent.clone());
                self.agents.agent_loop = Some(agent);

                if let Some(rx) = receiver {
                    self.spawn_agent_forwarder(new_ws_id.clone(), rx);
                }
            }
        } else {
            self.agents.agent_loop = self.agents.workspace_agents.get(&new_ws_id).cloned();
        }

        Ok(())
    }
}
