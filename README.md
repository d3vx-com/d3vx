# d3vx

A Rust-based terminal application for AI-assisted software engineering. Unlike generic AI coding assistants that operate as simple chat tools, d3vx is built around **task-oriented workflows** — chat is the interface, but isolated background agents, a 7-phase pipeline, and parallel orchestration are the engine.

## What makes d3vx different

| Feature | d3vx | Typical AI coding tools |
|---------|------|------------------------|
| **Background task execution** | `--vex` launches isolated agents in git worktrees without blocking your chat | All work happens in one session, blocking interaction |
| **Parallel agent orchestration** | Coordinator dispatches bounded child agents with dependency graphs, winner selection, and tie-break evaluation | Sequential or manual multi-tab workflows |
| **7-phase pipeline** | Research → Ideation → Plan → Draft → Review → Implement → Docs — structured progression | Single-prompt → output |
| **Task persistence** | SQLite-backed task records power kanban, list views, and auto-resume | Ephemeral conversations only |
| **Model routing** | Auto-routes cheap/standard/premium models per task type and focus mode | Single model per session |
| **Doom loop detection** | Detects repetitive agent patterns and intervenes | No self-monitoring |
| **Context compaction** | Auto-summarizes history when approaching token limits | Manual truncation or context loss |
| **MCP support** | Global and project-scoped Model Context Protocol servers | Configured per-session only |

## Quick Start

```bash
cargo run
```

Set an API key before running:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
cargo run
```

Run the setup wizard on first use for guided provider selection:

```bash
cargo run -- setup
```

Or start in a specific view:

```bash
cargo run -- --ui kanban
cargo run -- --ui list
```

## Core Features

### Chat-First Interface with Focus Modes

- `Chat`: normal mixed work
- `Build`: implementation-first
- `Plan`: read-first and decomposition-first
- `Docs`: documentation and developer guidance
- `Test`: debugging, validation, regression coverage
- `Review`: risk detection, change inspection, merge readiness

Focus modes bias the assistant without creating a separate workflow — the active mode also influences model routing automatically.

### Background Isolated Task Execution (`--vex`)

Launch work in git worktrees without leaving your current conversation:

```text
refactor the pipeline scheduler --vex --review
resolve issue #42 --vex --review --merge
prepare migration notes --vex --docs
analyze what this codebase is about --parallel
```

What happens:
1. A background task is created through the orchestrator
2. An isolated workspace is provisioned via git worktree
3. The task is dispatched in the background
4. You stay in your current conversation
5. Monitor progress from the navigator or with `/agents`
6. If `--merge` is set, d3vx only merges if the branch passes safety checks

### Parallel Agent Orchestration

d3vx supports coordinated parallel agents, not raw concurrent workers:
- The main agent operates as a coordinator when parallel mode is enabled
- Delegated child agents run as bounded executors — no recursive spawning
- Child tasks carry dependency links; only ready tasks start immediately
- Results are summarized back into the parent session
- Batches can compare candidate outputs and select a winner with tie-break evaluation
- Orchestration graphs are persisted in task data, not just transient UI state

### Multi-Provider LLM Support

| Provider | `provider` value | API Key | Notes |
|----------|-----------------|---------|-------|
| **Anthropic** | `anthropic` | `ANTHROPIC_API_KEY` | Default, custom SSE protocol |
| **OpenAI** | `openai` | `OPENAI_API_KEY` | GPT-4o, GPT-4.1, o3-mini |
| **Groq** | `groq` | `GROQ_API_KEY` | Ultra-fast inference |
| **xAI** | `xai` | `XAI_API_KEY` | Grok 3 models |
| **Mistral** | `mistral` | `MISTRAL_API_KEY` | Codestral, Mistral Large |
| **DeepSeek** | `deepseek` | `DEEPSEEK_API_KEY` | DeepSeek V3, R1 reasoning |
| **Ollama** | `ollama` | *(none)* | Local models, no key needed |
| **OpenRouter** | `openrouter` | `OPENROUTER_API_KEY` | Any model via proxy |

Configure in `~/.d3vx/config.yml` (global) or `.d3vx/config.yml` (per-project).

### Task Views

**Kanban board** (`/board`): Persisted task records grouped into Inbox → Running → Review → Done → Failed.

**Task list** (`/list`): Persisted tasks with state, execution mode, priority, and title — includes an inspector for task details.

Both views surface parent/child batch graphs directly, not hidden in inspector panels.

### Advanced Features

- **Context compaction**: Auto-summarizes conversation history when approaching token limits
- **Doom loop detection**: Detects repetitive tool call patterns and provides warnings
- **Skills on demand**: Load specialized capabilities from SKILL.md files on-demand
- **Best-of-N pattern**: Generate multiple variants and select the best with a selector agent
- **LSP integration**: Full Language Server Protocol support — diagnostics, completion, goto-definition for TypeScript, Rust, Python, Go, Java, C/C++, C#
- **Plugin architecture**: Extend with custom tools, agents, hooks, providers, and UI extensions
- **Budget enforcement**: Per-session and per-day cost limits with warnings and automatic stopping
- **Planner/Executor split**: Plans generated by planner agent, gated through user approval before execution
- **GitHub automation**: Issue polling, PR creation, CI status tracking, review response loops
- **Telegram notifications**: Task completion and failure alerts
- **Claims-based authorization**: Fine-grained permission control with wildcard patterns
- **Web dashboard**: SSE-based real-time event streaming
- **Session snapshot & resume**: Auto-snapshots after each phase, auto-resume on startup
- **Reaction engine**: Automatic responses to CI failures, review comments, merge conflicts

## Configuration

- **Global config**: `~/.d3vx/config.yml`
- **Project config**: `.d3vx/config.yml` (overrides global)
- **Project context**: `.d3vx/project.md`
- **Agent rules**: `.d3vx/rules.yaml` or `.d3vx/project.md`
- **Database**: `~/.d3vx/d3vx.db` (SQLite)

### Budget Configuration

Budget enforcement prevents runaway API costs during autonomous execution. Configure in `~/.d3vx/config.yml`:

```yaml
budget:
  # Per-session limit in USD (0 = disabled)
  per_session: 5.00
  # Per-day limit in USD (0 = disabled)
  per_day: 50.00
  # Warn when spend reaches this fraction (0.0 - 1.0)
  warn_at: 0.8
  # Pause execution when limit reached (0.0 - 1.0)
  pause_at: 1.0
  # Enable budget enforcement
  enabled: true
```

**How it works:**
- At 80% of session budget → Warning logged
- At 100% of session budget → Agent stops automatically
- Applies to both TUI sessions and `--vex` background tasks
- Critical for overnight autonomous work — prevents stuck agents from burning credits

### MCP Configuration

**Global** (`~/.d3vx/config.yml`):

```yaml
mcp:
  servers:
    sqlite:
      command: "npx"
      args: ["-y", "@modelcontextprotocol/server-sqlite", "--db", "/path/to/global.sqlite"]
```

**Project-specific** (`.d3vx/config.yml`):

```yaml
mcp:
  servers:
    local-db:
      command: "python3"
      args: ["src/tools/mcp_server.py"]
```

Project-specific servers are merged with global ones; conflicts resolved in favor of project config.

## Architecture

At a high level, the runtime is built around:
- A TUI application state machine (ratatui)
- A pipeline orchestrator with task queue and worker pool
- A conversation manager with context compaction and doom loop detection
- 42 built-in tools (Bash, Read, Write, Edit, Glob, Grep, Skill, MCP, and more)
- Task persistence in SQLite
- Isolated workspaces for `--vex` execution
- Poller-driven GitHub intake
- Daemon-backed background execution and recovery

## What Still Needs Work

- Full end-to-end test stabilization
- Broader CLI implementation beyond the TUI
- More complete kanban/list interactions
- Production hardening around recovery
- Hosted webhook mode
- Fully automatic conflict repair across all execution modes

## Development Status

- **Architecturally**: Substantial — ~126K lines of Rust across 643 files, 42 tools, 1379+ tests
- **Functionally**: Usable in the TUI with real background task execution
- **Production readiness**: Not yet — beyond prototype stage but not at broad team rollout level

You can use d3vx today if you want an experimental but serious AI coding TUI with background isolated task execution. Do not treat it as production-ready for enterprise deployment.

## License

d3vx is provided under the **PolyForm Noncommercial** license with a **commercial permission exception**.

- **Personal, educational, and research use**: Free to use, modify, and distribute
- **Commercial / business use**: **Requires prior written permission** from the copyright holder

This includes any use in commercial products, SaaS offerings, or for-profit deployments.

See [LICENSE](LICENSE) for the full text. For commercial licensing inquiries, contact the project maintainers.
