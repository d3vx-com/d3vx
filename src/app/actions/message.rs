//! Message sending and execution actions

use anyhow::Result;
use std::time::Instant;

use crate::app::slash_commands;
use crate::app::App;
use crate::event::Event;
use crate::ipc::{Message, ThinkingState};

use super::mentions::{
    apply_focus_mode_to_prompt, extract_image_paths, resolve_file_mentions, MentionResolution,
};
use super::parse_message_execution_flags;

impl App {
    /// Add a system message to the message list
    pub fn add_system_message(&mut self, content: &str) {
        self.session.messages.push(Message::system(content));
    }

    /// Send the current input buffer as a message
    pub fn send_message(&mut self) -> Result<()> {
        let content = std::mem::take(&mut self.ui.input_buffer);
        if content.trim().is_empty() {
            return Ok(());
        }
        self.ui.cursor_position = 0;

        // If currently busy, queue the message
        if self.session.thinking.is_thinking {
            self.session.message_queue.push(content);
            self.add_system_message("Message queued.");
            return Ok(());
        }

        self.execute_message(content)
    }

    /// Execute a message immediately
    pub fn execute_message(&mut self, mut content: String) -> Result<()> {
        // Detect drag-and-dropped images (only if not a slash command)
        if !content.starts_with('/') {
            let (remaining, dropped_images) = extract_image_paths(&content, self.cwd.as_deref());
            if !dropped_images.is_empty() {
                for img in dropped_images {
                    self.session.pending_images.push(img);
                }
                content = remaining;
                let count = self.session.pending_images.len();
                self.add_system_message(&format!(
                    "{} overall images attached (including drag-and-dropped).",
                    count
                ));
            }
        }

        // Add to history
        if !content.trim().is_empty()
            && (self.ui.input_history.is_empty() || self.ui.input_history.last() != Some(&content))
        {
            self.ui.input_history.push(content.clone());
        }
        self.ui.history_index = self.ui.input_history.len();

        // Handle Slash Commands (Ephemeral - not saved to conversation history)
        if content.starts_with('/') {
            if slash_commands::try_execute_slash_command(self, &content)? {
                return Ok(());
            }
        }

        // Parse execution-policy flags for chat-first task execution.
        if !content.starts_with('/') {
            let (description, flags) = parse_message_execution_flags(&content);
            let mention_resolution = resolve_file_mentions(&description, self.cwd.as_deref());
            if !mention_resolution.resolved_paths.is_empty() {
                self.add_system_message(&format!(
                    "Attached file context: {}",
                    mention_resolution.resolved_paths.join(", ")
                ));
            }
            if !mention_resolution.unresolved.is_empty() {
                self.add_system_message(&format!(
                    "Could not resolve file mentions: {}",
                    mention_resolution.unresolved.join(", ")
                ));
            }

            // Enable parallel agents if --parallel flag is present
            if flags.parallel_agents() {
                self.agents.parallel_agents_enabled = true;
                self.add_system_message("Parallel agent mode enabled for this task.");
            }

            if flags.requires_background_task() {
                if !mention_resolution.expanded_prompt.trim().is_empty() {
                    let focused_prompt = apply_focus_mode_to_prompt(
                        &mention_resolution.expanded_prompt,
                        self.ui.focus_mode,
                    );
                    self.start_vex_task_with_flags(&focused_prompt, flags)?;
                } else {
                    self.add_system_message("Task flags require a task description.");
                }
                return Ok(());
            }
        }

        // Add user message to local state
        let display_content =
            if !self.session.pending_images.is_empty() && content.trim().is_empty() {
                format!("[Sent {} image(s)]", self.session.pending_images.len())
            } else {
                content.clone()
            };
        let user_msg = Message::user(&display_content);
        self.session.messages.push(user_msg);

        // Immediate save for persistence
        if self.agents.agent_loop.is_some() {
            if let Some(tx) = &self.event_tx {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(Event::SaveSession).await;
                });
            }
        }

        // Start thinking indicator
        self.session.thinking_start = Some(Instant::now());

        // Handle Direct Bash Execution (!)
        if content.starts_with('!') {
            return super::shell::execute_bash_command(self, &content);
        }

        let mention_resolution = if content.starts_with('/') {
            MentionResolution {
                expanded_prompt: content.clone(),
                resolved_paths: Vec::new(),
                unresolved: Vec::new(),
            }
        } else {
            resolve_file_mentions(&content, self.cwd.as_deref())
        };
        let focused_prompt =
            apply_focus_mode_to_prompt(&mention_resolution.expanded_prompt, self.ui.focus_mode);
        if !mention_resolution.resolved_paths.is_empty() {
            self.add_system_message(&format!(
                "Attached file context: {}",
                mention_resolution.resolved_paths.join(", ")
            ));
        }
        if !mention_resolution.unresolved.is_empty() {
            self.add_system_message(&format!(
                "Could not resolve file mentions: {}",
                mention_resolution.unresolved.join(", ")
            ));
        }

        self.session.thinking = ThinkingState {
            is_thinking: true,
            text: String::new(),
            phase: crate::ipc::types::ThinkingPhase::Thinking,
        };

        // Send via IPC or AgentLoop
        if let Some(ref client) = self.ipc_client {
            let client = client.clone();
            let content_clone = focused_prompt.clone();
            tokio::spawn(async move {
                let _ = client.send_message(&content_clone).await;
            });
        } else if let Some(ref agent) = self.agents.agent_loop {
            let agent = agent.clone();
            let content_clone = focused_prompt;
            let config = self.config.clone();
            let explicit_model = self.model.clone();
            let focus_mode = self.ui.focus_mode;
            let parallel_enabled = self.agents.parallel_agents_enabled;
            let pending_images: Vec<std::path::PathBuf> =
                std::mem::take(&mut self.session.pending_images);

            tokio::spawn(async move {
                {
                    let mut agent_config = agent.config.write().await;
                    agent_config.model = crate::app::agent::resolve_routed_model(
                        &config,
                        explicit_model.as_deref(),
                        focus_mode,
                        Some(&content_clone),
                        agent_config.plan_mode,
                        parallel_enabled || agent_config.allow_parallel_spawn,
                    );
                }
                if pending_images.is_empty() {
                    agent.add_user_message(&content_clone).await;
                } else {
                    let mut blocks = Vec::new();
                    if !content_clone.trim().is_empty() {
                        blocks.push(crate::providers::ContentBlock::text(&content_clone));
                    }
                    for img_path in pending_images {
                        if let Ok(bytes) = std::fs::read(&img_path) {
                            use base64::Engine;
                            let base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

                            let extension = img_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("png");
                            let media_type = match extension.to_lowercase().as_str() {
                                "jpg" | "jpeg" => "image/jpeg",
                                "png" => "image/png",
                                "gif" => "image/gif",
                                "webp" => "image/webp",
                                _ => "image/jpeg",
                            };

                            blocks.push(crate::providers::ContentBlock::Image {
                                source: crate::providers::ImageSource {
                                    source_type: "base64".to_string(),
                                    media_type: media_type.to_string(),
                                    data: base64,
                                },
                            });
                        }
                    }
                    agent.add_user_blocks(blocks).await;
                }
            });
            self.run_agent_loop();
        } else {
            self.session.messages.push(Message::error(
                "Agent not initialized. Please check your API key in ~/.d3vx/config.yaml",
            ));
            self.session.thinking = ThinkingState::default();
            self.session.thinking_start = None;
        }

        Ok(())
    }
}
