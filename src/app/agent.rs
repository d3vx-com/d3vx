//! Agent Management Logic

use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use tracing::{error, info};

use crate::agent::{AgentConfig, AgentEvent, AgentLoop, ToolCoordinator};
use crate::app::App;
use crate::app::FocusMode;
use crate::config::{
    format_provider_options, get_provider_config, get_setup_instructions, load_config,
    LoadConfigOptions,
};
use crate::providers::anthropic::AnthropicProvider;
use crate::providers::openai_compatible::{
    deepseek_config, groq_config, mistral_config, ollama_config, openai_config, openrouter_config,
    xai_config, OpenAICompatibleProvider,
};
use crate::tools::AgentRole;

fn main_agent_roles(parallel_enabled: bool) -> (AgentRole, crate::agent::prompt::Role) {
    if parallel_enabled {
        (AgentRole::TechLead, crate::agent::prompt::Role::TechLead)
    } else {
        (AgentRole::Executor, crate::agent::prompt::Role::Executor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoutingPhase {
    Planning,
    Implementation,
    Review,
    Docs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskComplexity {
    Trivial,
    Standard,
    Complex,
}

fn infer_routing_phase(focus_mode: FocusMode, plan_mode: bool) -> RoutingPhase {
    if plan_mode {
        return RoutingPhase::Planning;
    }

    match focus_mode {
        FocusMode::Chat | FocusMode::Build => RoutingPhase::Implementation,
        FocusMode::Plan => RoutingPhase::Planning,
        FocusMode::Docs => RoutingPhase::Docs,
        FocusMode::Test | FocusMode::Review => RoutingPhase::Review,
    }
}

fn infer_task_complexity(
    prompt: &str,
    focus_mode: FocusMode,
    parallel_enabled: bool,
) -> TaskComplexity {
    let normalized = prompt.to_lowercase();
    let token_count = normalized.split_whitespace().count();
    let mention_count = normalized.matches('@').count();

    let complex_signals = [
        "parallel agent",
        "parallel agents",
        "multi-agent",
        "multiple agents",
        "codebase",
        "architecture",
        "refactor",
        "end-to-end",
        "full repo",
        "whole project",
    ];
    if parallel_enabled
        || mention_count >= 3
        || token_count >= 80
        || complex_signals
            .iter()
            .any(|signal| normalized.contains(signal))
    {
        return TaskComplexity::Complex;
    }

    let trivial_signals = ["explain", "summarize", "review", "docs", "rename", "typo"];
    if token_count <= 18
        && mention_count <= 1
        && !normalized.contains('\n')
        && matches!(
            focus_mode,
            FocusMode::Chat | FocusMode::Docs | FocusMode::Review
        )
        && trivial_signals
            .iter()
            .any(|signal| normalized.contains(signal))
    {
        return TaskComplexity::Trivial;
    }

    TaskComplexity::Standard
}

fn configured_provider_models<'a>(
    config: &'a crate::config::D3vxConfig,
) -> Option<&'a crate::config::types::ProviderConfig> {
    config
        .providers
        .configs
        .as_ref()
        .and_then(|configs| configs.get(&config.provider))
}

pub(crate) fn resolve_routed_model(
    config: &crate::config::D3vxConfig,
    explicit_model: Option<&str>,
    focus_mode: FocusMode,
    prompt: Option<&str>,
    plan_mode: bool,
    parallel_enabled: bool,
) -> String {
    if let Some(model) = explicit_model.filter(|value| !value.trim().is_empty()) {
        return model.to_string();
    }

    let (default_model, _, _) = get_provider_config(config);
    let Some(routing) = config
        .model_routing
        .as_ref()
        .filter(|routing| routing.enabled)
    else {
        return default_model;
    };

    let prompt = prompt.unwrap_or_default();
    let complexity = infer_task_complexity(prompt, focus_mode, parallel_enabled);
    let provider_models = configured_provider_models(config);

    if routing.complexity_routing && matches!(complexity, TaskComplexity::Trivial) {
        if let Some(model) = routing
            .cheap_model
            .clone()
            .or_else(|| provider_models.and_then(|provider| provider.cheap_model.clone()))
        {
            return model;
        }
    }

    let phase_model = match infer_routing_phase(focus_mode, plan_mode) {
        RoutingPhase::Planning => routing
            .premium_model
            .clone()
            .or_else(|| provider_models.and_then(|provider| provider.research_model.clone())),
        RoutingPhase::Implementation => routing.standard_model.clone(),
        RoutingPhase::Review => routing.premium_model.clone(),
        RoutingPhase::Docs => routing
            .standard_model
            .clone()
            .or_else(|| routing.premium_model.clone()),
    };

    phase_model
        .or_else(|| routing.standard_model.clone())
        .or_else(|| provider_models.map(|provider| provider.default_model.clone()))
        .unwrap_or(default_model)
}

impl App {
    /// Create a provider based on current configuration.
    ///
    /// Dispatches to the correct provider implementation based on `config.provider`.
    /// All OpenAI-compatible providers reuse a single implementation (DRY).
    pub fn create_provider(
        config: &crate::config::D3vxConfig,
    ) -> Result<Arc<dyn crate::providers::Provider>> {
        let (_resolved_model, api_key, base_url) = get_provider_config(config);

        let provider_id = config.provider.to_lowercase();

        match provider_id.as_str() {
            // Anthropic has its own unique API protocol
            "anthropic" => {
                let Some(api_key) = api_key else {
                    let instructions = get_setup_instructions("anthropic");
                    anyhow::bail!(
                        "No API key found for Anthropic.\n\n{}\n\n{}",
                        instructions.trim(),
                        format_provider_options()
                    );
                };
                let provider_options = crate::providers::ProviderOptions {
                    base_url,
                    ..Default::default()
                };
                Ok(Arc::new(AnthropicProvider::with_options(
                    api_key,
                    provider_options,
                )))
            }

            // All OpenAI-compatible providers share one implementation
            "openai" => {
                let Some(api_key) = api_key else {
                    let instructions = get_setup_instructions("openai");
                    anyhow::bail!(
                        "No API key found for OpenAI.\n\n{}\n\n{}",
                        instructions.trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(openai_config(
                    api_key, base_url,
                ))))
            }
            "groq" => {
                let Some(api_key) = api_key else {
                    anyhow::bail!(
                        "No API key found for Groq.\n\n{}\n\n{}",
                        get_setup_instructions("groq").trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(groq_config(
                    api_key, base_url,
                ))))
            }
            "xai" => {
                let Some(api_key) = api_key else {
                    anyhow::bail!(
                        "No API key found for xAI.\n\n{}\n\n{}",
                        get_setup_instructions("xai").trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(xai_config(
                    api_key, base_url,
                ))))
            }
            "mistral" => {
                let Some(api_key) = api_key else {
                    anyhow::bail!(
                        "No API key found for Mistral.\n\n{}\n\n{}",
                        get_setup_instructions("mistral").trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(mistral_config(
                    api_key, base_url,
                ))))
            }
            "deepseek" => {
                let Some(api_key) = api_key else {
                    anyhow::bail!(
                        "No API key found for DeepSeek.\n\n{}\n\n{}",
                        get_setup_instructions("deepseek").trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(deepseek_config(
                    api_key, base_url,
                ))))
            }
            "ollama" => {
                // Ollama is local — no API key required
                Ok(Arc::new(OpenAICompatibleProvider::new(ollama_config(
                    base_url,
                ))))
            }
            "openrouter" => {
                let Some(api_key) = api_key else {
                    anyhow::bail!(
                        "No API key found for OpenRouter.\n\n{}\n\n{}",
                        get_setup_instructions("openrouter").trim(),
                        format_provider_options()
                    );
                };
                Ok(Arc::new(OpenAICompatibleProvider::new(openrouter_config(
                    api_key, base_url,
                ))))
            }

            _ => {
                anyhow::bail!(
                    "Unknown provider '{}'.\n\n{}\n\nSupported providers: anthropic, openai, groq, xai, mistral, deepseek, ollama, openrouter",
                    provider_id,
                    format_provider_options()
                );
            }
        }
    }

    /// Create an agent loop for standalone mode
    pub fn create_agent(
        cwd: &Option<String>,
        model: &Option<String>,
        session_id: &Option<String>,
        tools: Arc<ToolCoordinator>,
        plan_mode: bool,
        parallel_agents: bool,
        focus_mode: FocusMode,
        permission_manager: Option<Arc<crate::pipeline::permission::PermissionManager>>,
        db: Option<crate::store::database::DatabaseHandle>,
    ) -> Result<(
        Option<Arc<AgentLoop>>,
        Option<tokio::sync::broadcast::Receiver<AgentEvent>>,
        Option<String>,
    )> {
        // Load config
        let config = match load_config(LoadConfigOptions {
            project_root: cwd.clone(),
            ..Default::default()
        }) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Failed to load config: {}", e);
                return Ok((
                    None,
                    None,
                    Some(format!("Config error: {}. Run /doctor for details.", e)),
                ));
            }
        };

        let working_dir = cwd.clone().unwrap_or_else(|| ".".to_string());

        // Use passed parallel_agents value (from App), falling back to config
        let parallel_enabled = parallel_agents || config.subagent.parallel_agents;

        // Create provider
        let provider = match Self::create_provider(&config) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to create provider: {}", e);
                return Ok((None, None, Some(e.to_string())));
            }
        };

        // Build agent config
        let (agent_role, prompt_role) = main_agent_roles(parallel_enabled);
        let agent_config = AgentConfig {
            model: resolve_routed_model(
                &config,
                model.as_deref(),
                focus_mode,
                None,
                plan_mode,
                parallel_enabled,
            ),
            system_prompt: crate::agent::prompt::build_system_prompt_with_options(
                &working_dir,
                Some(&prompt_role),
                parallel_enabled,
            ),
            working_dir: working_dir.clone(),
            session_id: session_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            allow_parallel_spawn: parallel_enabled,
            role: agent_role,
            plan_mode,
            db: db.clone(),
            budget: config.budget.clone(),
            ..Default::default()
        };

        // Create CommandGuard with permission manager if available
        let guard = if let Some(ref pm) = permission_manager {
            let session_id_for_guard = session_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            Arc::new(crate::agent::guard::CommandGuard::with_permission_manager(
                config.permissions.clone(),
                agent_config.session_id.clone(),
                session_id_for_guard,
                pm.clone(),
            ))
        } else {
            Arc::new(crate::agent::guard::CommandGuard::new(
                config.permissions.clone(),
                agent_config.session_id.clone(),
            ))
        };

        let (mut agent, rx) = AgentLoop::with_events(provider, tools, Some(guard), agent_config);

        // Attach LSP bridge if configured
        if let Some(ref lsp_config) = config.lsp {
            if lsp_config.enabled {
                let bridge_configs: Vec<crate::lsp::LspBridgeConfig> = lsp_config
                    .servers
                    .values()
                    .map(|s| crate::lsp::LspBridgeConfig {
                        binary: s.command.first().cloned().unwrap_or_default(),
                        args: s.command[1..].to_vec(),
                        extensions: s.extensions.clone(),
                    })
                    .collect();
                if !bridge_configs.is_empty() {
                    let root = std::path::PathBuf::from(&working_dir);
                    agent = agent.with_lsp_bridge(Arc::new(crate::lsp::LspBridge::new(
                        bridge_configs,
                        root,
                    )));
                }
            }
        }

        Ok((Some(Arc::new(agent)), Some(rx), None))
    }

    /// Run the agent loop (standalone mode) and handle state updates
    pub fn run_agent_loop(&mut self) {
        let Some(agent) = self.agents.agent_loop.clone() else {
            return;
        };

        // Start thinking indicator
        self.session.thinking_start = Some(Instant::now());
        self.session.thinking = crate::ipc::ThinkingState {
            is_thinking: true,
            text: String::new(),
            phase: crate::ipc::types::ThinkingPhase::Thinking,
        };

        // Run the agent to get a response
        tokio::spawn(async move {
            match agent.run().await {
                Ok(result) => {
                    if let Some(reason) = result.safety_stop_reason() {
                        error!("Agent stopped for safety: {reason}");
                    } else {
                        info!(
                            "Agent completed: {} chars, {} tool calls",
                            result.text.len(),
                            result.tool_calls
                        );
                    }
                }
                Err(e) => {
                    error!("Agent run failed: {}", e);
                }
            }
        });
    }

    /// Synchronize agent context with the currently selected workspace
    pub async fn sync_agent_context(&self) -> Result<()> {
        let Some(task) = self.workspaces.get(self.workspace_selected_index) else {
            return Ok(());
        };

        if let Some(agent) = &self.agents.agent_loop {
            let mut config = agent.config.write().await;
            if config.working_dir != task.path {
                info!("Switching agent working directory to: {}", task.path);
                config.working_dir = task.path.clone();
                // Update system prompt and role for the new directory
                let (agent_role, prompt_role) = main_agent_roles(config.allow_parallel_spawn);
                config.role = agent_role;
                config.system_prompt = crate::agent::prompt::build_system_prompt_with_options(
                    &task.path,
                    Some(&prompt_role),
                    config.allow_parallel_spawn,
                );
            }
            config.model = resolve_routed_model(
                &self.config,
                self.model.as_deref(),
                self.ui.focus_mode,
                None,
                config.plan_mode,
                config.allow_parallel_spawn,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ModelRouting;

    #[test]
    fn main_agent_uses_executor_role_without_parallel_mode() {
        let (runtime_role, prompt_role) = main_agent_roles(false);
        assert_eq!(runtime_role, AgentRole::Executor);
        assert_eq!(prompt_role, crate::agent::prompt::Role::Executor);
    }

    #[test]
    fn main_agent_uses_tech_lead_role_with_parallel_mode() {
        let (runtime_role, prompt_role) = main_agent_roles(true);
        assert_eq!(runtime_role, AgentRole::TechLead);
        assert_eq!(prompt_role, crate::agent::prompt::Role::TechLead);
    }

    #[test]
    fn routed_model_prefers_trivial_model_for_small_review_tasks() {
        let mut config = crate::config::defaults::default_config();
        config.model = "baseline".to_string();
        config.model_routing = Some(ModelRouting {
            enabled: true,
            cheap_model: Some("cheap-model".to_string()),
            standard_model: Some("impl-model".to_string()),
            premium_model: Some("review-model".to_string()),
            complexity_routing: true,
        });

        let model = resolve_routed_model(
            &config,
            None,
            FocusMode::Review,
            Some("review @src/main.rs"),
            false,
            false,
        );
        assert_eq!(model, "cheap-model");
    }

    #[test]
    fn routed_model_prefers_planning_model_in_plan_mode() {
        let mut config = crate::config::defaults::default_config();
        config.model_routing = Some(ModelRouting {
            enabled: true,
            cheap_model: Some("cheap-model".to_string()),
            standard_model: Some("impl-model".to_string()),
            premium_model: Some("planner-model".to_string()),
            complexity_routing: true,
        });

        let model = resolve_routed_model(
            &config,
            None,
            FocusMode::Plan,
            Some("analyze the scheduler and propose a plan"),
            true,
            false,
        );
        assert_eq!(model, "planner-model");
    }
}
