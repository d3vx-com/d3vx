//! Tools Module
//!
//! This module provides the tool system for d3vx, implementing the same tools
//! as the TypeScript version but in Rust for better performance.

#[cfg(test)]
mod tests;

pub mod background_task;
pub mod bash;
pub mod best_of_n_tool;
pub mod command_classifier;
pub mod complete;
pub mod cron;
pub mod delegate_review;
pub mod dispatch;
pub mod draft_tool;
pub mod edit;
pub mod file_tracker;
pub mod glob;
pub mod grep;
pub mod inbox;
pub mod job_board;
pub mod mcp;
pub mod mcp_resources;
pub mod multi_edit;
pub mod multi_strategy;
pub mod notebook_edit;
pub mod plan_mode;
pub mod question;
pub mod read;
pub mod registry;
pub mod sandbox;
pub mod skill;
pub mod spawn_parallel;
pub mod structured_output;
pub mod team_ops;
pub mod text_match;
pub mod think;
pub mod todo;
pub mod tool_access;
pub mod types;
pub mod web_fetch;
pub mod web_search;
pub mod worktree;
pub mod write;

// Re-exports
pub use background_task::{TaskOutputTool, TaskStopTool};
pub use bash::BashTool;
pub use best_of_n_tool::BestOfNTool;
pub use command_classifier::{classify_command, CommandSafety};
pub use complete::CompleteTaskTool;
pub use cron::{CronCreateTool, CronDeleteTool, CronListTool};
pub use delegate_review::DelegateReviewTool;
pub use dispatch::RelayMessageTool;
pub use draft_tool::DraftChangeTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use inbox::{ReadInboxTool, SendInboxMessageTool};
pub use job_board::{
    CreateJobTool, GetJobTool, HaltJobTool, ListJobsTool, ReadJobOutputTool, UpdateJobTool,
};
pub use mcp::McpTool;
pub use mcp_resources::{ListMcpResourcesTool, ReadMcpResourceTool};
pub use multi_edit::MultiEditTool;
pub use multi_strategy::MultiStrategyTool;
pub use notebook_edit::NotebookEditTool;
pub use plan_mode::{EnterPlanModeTool, ExitPlanModeTool};
pub use question::QuestionTool;
pub use read::ReadTool;
pub use registry::{execute_tool, get_tool, list_tools, register_tool};
pub use sandbox::{
    execute_in_sandbox, platform_executor, ProcessSandbox, SandboxError, SandboxResult,
};
pub use skill::{SkillTool, SkillToolConfig};
pub use spawn_parallel::{SpawnParallelEvent, SpawnParallelTool, SpawnTask};
pub use structured_output::{schemas, SchemaValidator, StructuredOutputTool, ValidationError};
pub use team_ops::{DisbandSwarmTool, FormSwarmTool};
pub use think::ThinkTool;
pub use todo::TodoWriteTool;
pub use tool_access::{
    default_role_config, AgentRole, RoleToolConfig, RolesConfig, ToolAccessError,
    ToolAccessValidator,
};
pub use types::{SwarmContext, Tool, ToolContext, ToolDefinition, ToolResult};
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
pub use worktree::{WorktreeCreateTool, WorktreeRemoveTool};
pub use write::WriteTool;

/// Register all core tools
pub fn register_core_tools() {
    register_tool(BashTool::new());
    register_tool(ReadTool::new());
    register_tool(WriteTool::new());
    register_tool(EditTool::new());
    register_tool(GlobTool::new());
    register_tool(GrepTool::new());
    register_tool(ThinkTool::new());
    register_tool(QuestionTool::new());
    register_tool(TodoWriteTool::new());
    register_tool(WebFetchTool::new());
    register_tool(MultiEditTool::new());
    register_tool(DelegateReviewTool::new());
    register_tool(SendInboxMessageTool::new());
    register_tool(ReadInboxTool::new());
    register_tool(CompleteTaskTool::new());
    register_tool(DraftChangeTool::new());
    register_tool(SkillTool::new());
    register_tool(StructuredOutputTool::new());
    register_tool(BestOfNTool::new());
    register_tool(SpawnParallelTool::new());
    register_tool(MultiStrategyTool::new());
    register_tool(EnterPlanModeTool::new());
    register_tool(ExitPlanModeTool::new());
    register_tool(RelayMessageTool::new());
    register_tool(FormSwarmTool::new());
    register_tool(DisbandSwarmTool::new());
    register_tool(WorktreeCreateTool::new());
    register_tool(WorktreeRemoveTool::new());
    // Web & Search
    register_tool(WebSearchTool::new());
    register_tool(NotebookEditTool::new());
    register_tool(ListMcpResourcesTool::new());
    register_tool(ReadMcpResourceTool::new());
    // Job board
    register_tool(CreateJobTool::new());
    register_tool(UpdateJobTool::new());
    register_tool(ListJobsTool::new());
    register_tool(GetJobTool::new());
    register_tool(ReadJobOutputTool::new());
    register_tool(HaltJobTool::new());
    // Cron
    register_tool(CronCreateTool::new());
    register_tool(CronDeleteTool::new());
    register_tool(CronListTool::new());
    // Background tasks
    register_tool(TaskOutputTool::new());
    register_tool(TaskStopTool::new());
}
