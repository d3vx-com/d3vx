//! Vex Mode Tool Registration
//!
//! Helper for creating ToolCoordinator with core tools for autonomous vex agents.

use std::sync::Arc;

use crate::agent::prompt;
use crate::agent::prompt::Role;
use crate::agent::tool_coordinator::ToolCoordinator;
use crate::agent::{AgentConfig, AgentLoop};
use crate::config::D3vxConfig;
use crate::providers::Provider;
use crate::tools::AgentRole;

/// Build a ToolCoordinator with core tools for vex agents.
pub async fn build_vex_tools() -> Arc<ToolCoordinator> {
    let coordinator = Arc::new(ToolCoordinator::new());

    coordinator
        .register_tool(crate::tools::BashTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::ReadTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::WriteTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::EditTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::GlobTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::GrepTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::ThinkTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::WebFetchTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::WebSearchTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::TodoWriteTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::CompleteTaskTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::TaskOutputTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::TaskStopTool::new())
        .await;
    coordinator
        .register_tool(crate::tools::MultiEditTool::new())
        .await;

    coordinator
}

/// Create a provider from config.
pub fn create_provider(config: &D3vxConfig) -> anyhow::Result<Arc<dyn Provider>> {
    crate::app::App::create_provider(config)
}

/// Create an AgentLoop for vex execution.
pub fn create_vex_agent(
    config: &D3vxConfig,
    provider: Arc<dyn Provider>,
    tools: Arc<ToolCoordinator>,
    cwd: &str,
    session_id: &str,
    db: Option<crate::store::database::DatabaseHandle>,
) -> anyhow::Result<AgentLoop> {
    let agent_config = AgentConfig {
        model: config.model.clone(),
        system_prompt: prompt::build_system_prompt_with_options(cwd, Some(&Role::TechLead), false),
        working_dir: cwd.to_string(),
        session_id: session_id.to_string(),
        role: AgentRole::Executor,
        db,
        budget: config.budget.clone(),
        ..Default::default()
    };

    let guard = crate::agent::guard::CommandGuard::new(
        crate::config::PermissionsConfig::default(),
        session_id.to_string(),
    );

    Ok(AgentLoop::new(
        provider,
        tools,
        Some(Arc::new(guard)),
        agent_config,
    ))
}
