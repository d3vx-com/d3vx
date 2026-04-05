//! Session Management Logic

use anyhow::Result;
use rusqlite::params;
use tracing::debug;

use crate::app::App;
use crate::event::Event;
use crate::ipc::{Message, MessageRole};

impl App {
    /// Resume a session from the database
    pub async fn resume_session(&mut self, session_id: &str) -> Result<()> {
        let db_handle = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not available"))?;
        let db = db_handle.lock();
        let store = crate::store::session::SessionStore::from_connection(db.connection());

        let session = store
            .get(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;
        let session_metadata = session.metadata.clone();

        // Parse messages from JSON as IPC messages (full fidelity)
        let ui_messages: Vec<Message> = serde_json::from_str(&session.messages)?;

        // Convert back to provider messages for the AgentLoop (LLM context)
        let mut provider_messages = Vec::new();

        // Update CWD based on task relationship
        if let Some(ref task_id) = session.task_id {
            let task_path: Option<String> = db
                .connection()
                .query_row(
                    "SELECT worktree_path FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get::<_, String>(0),
                )
                .ok();

            if let Some(path) = task_path {
                self.cwd = Some(path);
            }
        } else {
            self.cwd = self.base_cwd.clone();
        }
        for msg in &ui_messages {
            match msg.role {
                MessageRole::User => {
                    provider_messages.push(crate::providers::Message::user_text(&msg.content));
                }
                MessageRole::Assistant => {
                    if msg.tool_calls.is_empty() {
                        provider_messages
                            .push(crate::providers::Message::assistant_text(&msg.content));
                    } else {
                        let mut blocks = Vec::new();
                        if !msg.content.is_empty() {
                            blocks.push(crate::providers::ContentBlock::text(&msg.content));
                        }
                        for tc in &msg.tool_calls {
                            blocks.push(crate::providers::ContentBlock::tool_use(
                                tc.id.clone(),
                                tc.name.clone(),
                                tc.input.clone(),
                            ));
                        }
                        provider_messages.push(crate::providers::Message::assistant_blocks(blocks));

                        // Also add any tool results as a separate user message if they exist
                        // This mirrors how providers expect them (Assistant tools -> User results)
                        let mut result_blocks = Vec::new();
                        for tc in &msg.tool_calls {
                            if let Some(output) = &tc.output {
                                result_blocks.push(crate::providers::ContentBlock::tool_result(
                                    tc.id.clone(),
                                    output.clone(),
                                ));
                            }
                        }
                        if !result_blocks.is_empty() {
                            provider_messages
                                .push(crate::providers::Message::user_blocks(result_blocks));
                        }
                    }
                }
                MessageRole::Shell => {
                    // Include shell commands as user context
                    let shell_content = format!(
                        "Ran shell command `{}`:\n{}",
                        msg.shell_cmd.as_deref().unwrap_or("?"),
                        msg.content
                    );
                    provider_messages.push(crate::providers::Message::user_text(shell_content));
                }
                MessageRole::System => {
                    // System messages are internal/UI mostly
                }
            }
        }

        // Update App state
        self.session.messages = ui_messages;
        self.session.session_id = Some(session_id.to_string());
        self.model = Some(session.model.clone());
        self.ui.show_welcome = false;

        // Update AgentLoop state if standalone
        if let Some(agent) = &self.agents.agent_loop {
            let mut conv = agent.conversation.write().await;
            conv.set_messages(provider_messages);

            // Also update the session ID and WORKING DIR in the agent config
            let mut config = agent.config.write().await;
            config.session_id = session_id.to_string();
            config.working_dir = self.cwd.clone().unwrap_or_else(|| ".".to_string());
        }

        // Explicitly drop database connection/lock before using self again
        drop(db);
        self.restore_parallel_batches_from_metadata(&session_metadata);

        self.add_system_message(&format!("Resumed session: {}", session_id));

        // Trigger save to update updated_at for this session
        if self.agents.agent_loop.is_some() {
            if let Some(tx) = &self.event_tx {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(Event::SaveSession).await;
                });
            }
        }

        Ok(())
    }

    /// Save the current session to the database
    pub async fn save_current_session(&mut self) -> Result<()> {
        let db_handle = match &self.db {
            Some(db) => db,
            None => {
                debug!("Database not available, skipping session save");
                return Ok(());
            }
        };

        // Determine session ID
        let session_id = if let Some(agent) = &self.agents.agent_loop {
            agent.config.read().await.session_id.clone()
        } else if let Some(sid) = &self.session.session_id {
            sid.clone()
        } else {
            debug!("No session ID available, generating a new one");
            let new_id = uuid::Uuid::new_v4().to_string();
            self.session.session_id = Some(new_id.clone());
            new_id
        };

        // Serialize messages
        let messages_json = serde_json::to_string(&self.session.messages)?;

        // Get provider from config if possible
        let provider = "anthropic".to_string();

        let model = self.model.clone().unwrap_or_else(|| "unknown".to_string());
        let token_count =
            (self.session.token_usage.input_tokens + self.session.token_usage.output_tokens) as i64;
        let project_path = self.cwd.clone();
        let session_metadata = serde_json::json!({
            "orchestration": self.agents.parallel_batches_metadata()
        })
        .to_string();

        // Lock database and save
        let db = db_handle.lock();
        let store = crate::store::session::SessionStore::from_connection(db.connection());

        // Determine task ID if we are in a satellite workspace
        let current_task_id = if self.workspace_selected_index < self.workspaces.len() {
            let ws = &self.workspaces[self.workspace_selected_index];
            if ws.id == "home" {
                None
            } else {
                Some(ws.id.clone())
            }
        } else {
            None
        };

        match store.get(&session_id)? {
            Some(_) => {
                // Update existing
                store.update(
                    &session_id,
                    crate::store::session::SessionUpdate {
                        messages: Some(messages_json),
                        token_count: Some(token_count),
                        summary: None,
                        metadata: Some(session_metadata.clone()),
                        state: None,
                    },
                )?;
                debug!("Session updated: {}", session_id);
            }
            None => {
                // Create new
                store.create(crate::store::session::NewSession {
                    id: Some(session_id.clone()),
                    task_id: current_task_id.clone(),
                    provider,
                    model,
                    messages: Some(messages_json),
                    token_count: Some(token_count),
                    summary: None,
                    project_path,
                    parent_session_id: None,
                    metadata: Some(session_metadata.clone()),
                    state: None,
                })?;
                debug!("New session created: {}", session_id);
            }
        }

        if let Some(task_id) = &current_task_id {
            let task_store = crate::store::task::TaskStore::from_connection(db.connection());
            if let Some(task) = task_store.get(task_id)? {
                let mut metadata = serde_json::from_str::<serde_json::Value>(&task.metadata)
                    .unwrap_or_else(|_| serde_json::json!({}));
                if let Some(map) = metadata.as_object_mut() {
                    map.insert(
                        "orchestration".to_string(),
                        self.agents.parallel_batches_metadata(),
                    );
                }
                task_store.update(
                    task_id,
                    crate::store::task::TaskUpdate {
                        metadata: Some(metadata),
                        ..Default::default()
                    },
                )?;
            }
        }

        Ok(())
    }

    /// Attempt automatic resume of the most recent session on startup.
    ///
    /// Looks for the latest session for the current project path.
    /// If found and valid, restores it. Returns true if a session was resumed.
    pub async fn try_auto_resume(&mut self) -> bool {
        let db_handle = match &self.db {
            Some(db) => db,
            None => {
                debug!("No database, skipping auto-resume");
                return false;
            }
        };

        let db = db_handle.lock();
        let store = crate::store::session::SessionStore::from_connection(db.connection());

        let latest = match store.get_latest(self.cwd.clone().as_ref().map(String::as_str)) {
            Ok(Some(s)) => s,
            Ok(None) => {
                debug!("No existing session found, skipping auto-resume");
                return false;
            }
            Err(e) => {
                debug!(error = %e, "Failed to query sessions for auto-resume");
                return false;
            }
        };

        // Validate with SessionRestorer before restoring
        let restorer = crate::recovery::SessionRestorer::new();
        if !restorer.validate_restoration(&latest) {
            debug!(
                session_id = %latest.id,
                state = ?latest.state,
                "Session not restorable, skipping auto-resume"
            );
            return false;
        }

        let session_id = latest.id.clone();
        drop(db);

        // Attempt to resume
        match self.resume_session(&session_id).await {
            Ok(()) => {
                debug!(session_id = %session_id, "Auto-resumed session");
                true
            }
            Err(e) => {
                debug!(error = %e, session_id = %session_id, "Auto-resume failed");
                false
            }
        }
    }
}
