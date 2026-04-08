//! CLI Argument Definitions using Clap derive macros
//!
//! This module defines the complete CLI structure for d3vx,
//! matching the TypeScript CLI in src/cli/index.ts.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// d3vx - The autonomous coding engine. Works while you sleep.
#[derive(Parser, Debug)]
#[command(
    name = "d3vx",
    author,
    version,
    about,
    long_about = "d3vx is a production-grade, provider-agnostic CLI tool for autonomous software engineering."
)]
pub struct Cli {
    /// LLM provider to use (anthropic, openai, ollama, groq)
    #[arg(short, long, global = true, env = "D3VX_PROVIDER")]
    pub provider: Option<String>,

    /// Model to use
    #[arg(short, long, global = true, env = "D3VX_MODEL")]
    pub model: Option<String>,

    /// Auto-approve all tool operations
    #[arg(long, global = true)]
    pub trust: bool,

    /// Bypass all required permissions and auto-approve everything
    #[arg(long, global = true)]
    pub bypass_permissions: bool,

    /// Disable streaming (buffer full response)
    #[arg(long, global = true)]
    pub no_stream: bool,

    /// Stream all raw LLM generation outputs to a file
    #[arg(long, global = true)]
    pub stream_out: Option<PathBuf>,

    /// Enable debug logging
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Output JSON instead of rendered text
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable parallel agent execution for complex tasks
    #[arg(long, global = true)]
    pub parallel_agents: bool,

    /// Run task in Vex mode (background, isolated worktree, autonomous)
    #[arg(long, global = true)]
    pub vex: bool,

    /// Resume last session
    #[arg(short, long, global = true)]
    pub r#continue: bool,

    /// Pick a session to resume (interactive)
    #[arg(short, long, global = true)]
    pub resume: bool,

    /// Resume specific session by ID
    #[arg(short, long, global = true)]
    pub session: Option<String>,

    /// UI mode (chat, kanban, list)
    #[arg(long, global = true, value_parser = ["chat", "kanban", "list"])]
    pub ui: Option<String>,

    /// Path to config file
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Working directory
    #[arg(long, global = true, env = "D3VX_CWD")]
    pub cwd: Option<PathBuf>,

    /// One-shot query (starts interactive REPL if empty)
    #[arg(global = true)]
    pub query: Option<String>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Initialize d3vx in the current project
    Init {
        /// Path to initialize (defaults to current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Interactive setup wizard for first-time configuration
    Setup {
        /// Provider to configure
        #[arg(short, long)]
        provider: Option<String>,
    },

    /// Check environment for required dependencies
    Doctor,

    /// Manage Telegram notifications
    Notify {
        #[command(subcommand)]
        action: NotifyAction,
    },

    /// Get/set/list configuration
    Config {
        /// Action to perform: get, set, list, delete
        action: String,

        /// Configuration key
        key: Option<String>,

        /// Configuration value (for set action)
        value: Option<String>,
    },

    /// Manage model pricing (list, refresh, get)
    Pricing {
        /// Action to perform: list, refresh, get
        action: String,

        /// Specific model to get pricing for
        model: Option<String>,
    },

    /// Show project and provider status
    Status,

    /// Execute the full SDLC pipeline (Research -> Plan -> Implement -> Validate)
    Implement {
        /// The task instruction to implement
        instruction: String,

        /// Skip research, do plan/implement/review
        #[arg(long)]
        fast: bool,

        /// Only run the implement phase
        #[arg(long)]
        quick: bool,

        /// Agent role (CODER, DOCUMENTER, REVIEWER)
        #[arg(long)]
        role: Option<String>,

        /// Queue the task for background Orchestrator Daemon
        #[arg(long)]
        queue: bool,
    },

    /// Manage isolated git worktrees for tasks
    Worktree {
        /// List all active worktrees
        #[arg(short, long)]
        list: bool,

        /// Show diff for task worktree
        #[arg(long)]
        review: Option<String>,

        /// Merge task worktree into main
        #[arg(long)]
        merge: Option<String>,

        /// Discard task worktree and branch
        #[arg(short, long)]
        discard: Option<String>,

        /// Find and recover crashed sessions
        #[arg(long)]
        recover: bool,
    },

    /// Resume an interrupted task pipeline process
    Resume {
        /// Task ID to resume
        task_id: Option<String>,
    },

    /// Manage hook presets
    Hooks {
        /// Action: list, add, remove, presets
        action: String,

        /// Additional arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Manage long-term memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Manage the background orchestrator daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Spawn background agents
    Spawn {
        #[command(subcommand)]
        action: SpawnAction,
    },

    /// Batch process tasks from a file
    Batch {
        #[command(subcommand)]
        action: BatchAction,
    },

    /// Generate and manage documentation
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },

    /// Run in fully autonomous mode
    Autonomous {
        #[command(subcommand)]
        action: AutonomousAction,
    },
}

/// Notify command actions
#[derive(Subcommand, Debug)]
pub enum NotifyAction {
    /// Interactive wizard to configure Telegram bot
    Setup,

    /// Send a test message to verify configuration
    Test,

    /// Remove Telegram notification config
    Disable,

    /// Show current notification configuration status
    Status,
}

/// Memory command actions
#[derive(Subcommand, Debug)]
pub enum MemoryAction {
    /// List all memories
    List {
        /// Filter by memory type
        #[arg(short, long)]
        r#type: Option<String>,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show a specific memory
    Show {
        /// Memory name or ID
        name: String,
    },

    /// Create a new memory
    Create {
        /// Memory name
        name: String,

        /// Memory type (user, feedback, project, reference)
        #[arg(short, long)]
        r#type: String,

        /// Memory content (reads from stdin if not provided)
        #[arg(short, long)]
        content: Option<String>,
    },

    /// Update an existing memory
    Update {
        /// Memory name or ID
        name: String,

        /// New content (reads from stdin if not provided)
        #[arg(short, long)]
        content: Option<String>,
    },

    /// Delete a memory
    Delete {
        /// Memory name or ID
        name: String,
    },

    /// Search memories
    Search {
        /// Search query
        query: String,
    },
}

/// Daemon command actions
#[derive(Subcommand, Debug)]
pub enum DaemonAction {
    /// Start the daemon
    Start {
        /// Run in background (detach)
        #[arg(short, long)]
        detach: bool,
    },

    /// Stop the daemon
    Stop {
        /// Force kill if graceful shutdown fails
        #[arg(short, long)]
        force: bool,
    },

    /// Show daemon status
    Status,

    /// Show daemon logs
    Logs {
        /// Follow logs in real-time
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long)]
        lines: Option<usize>,
    },

    /// Restart the daemon
    Restart {
        /// Run in background (detach)
        #[arg(short, long)]
        detach: bool,
    },
}

/// Spawn command actions
#[derive(Subcommand, Debug)]
pub enum SpawnAction {
    /// Spawn a new background agent
    New {
        /// Task instruction
        instruction: String,

        /// Agent role
        #[arg(short, long)]
        role: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,
    },

    /// List spawned agents
    List,

    /// Show spawned agent status
    Status {
        /// Agent ID
        agent_id: Option<String>,
    },

    /// Stop a spawned agent
    Stop {
        /// Agent ID
        agent_id: String,
    },
}

/// Batch command actions
#[derive(Subcommand, Debug)]
pub enum BatchAction {
    /// Run batch tasks from a file
    Run {
        /// Path to tasks file
        file: PathBuf,

        /// Parallel execution
        #[arg(short, long)]
        parallel: bool,

        /// Maximum parallel tasks
        #[arg(short, long)]
        max_parallel: Option<usize>,
    },

    /// Validate a batch file without executing
    Validate {
        /// Path to tasks file
        file: PathBuf,
    },

    /// Show batch execution status
    Status {
        /// Batch ID
        batch_id: Option<String>,
    },
}

/// Docs command actions
#[derive(Subcommand, Debug)]
pub enum DocsAction {
    /// Generate documentation
    Generate {
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Format (markdown, html)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Watch for changes and regenerate
    Watch {
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Autonomous command actions
#[derive(Subcommand, Debug)]
pub enum AutonomousAction {
    /// Start autonomous mode
    Start {
        /// Maximum iterations (0 = unlimited)
        #[arg(short, long)]
        max_iterations: Option<usize>,

        /// Run in background
        #[arg(short, long)]
        detach: bool,
    },

    /// Stop autonomous mode
    Stop,

    /// Show autonomous mode status
    Status,
}

/// Parse CLI arguments from environment
pub fn parse_args() -> Cli {
    Cli::parse()
}

/// Parse CLI arguments from a slice of strings (useful for testing)
pub fn parse_from<I, T>(itr: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::parse_from(itr)
}
