# d3vx

> Autonomous coding agent in your terminal. Background isolated tasks, a 7-phase pipeline, a live dashboard, and multi-provider LLM support — all inside a Rust TUI.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built_with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)]()

---

d3vx is a terminal-first AI coding agent. It's spiritually influenced by [Claude Code](https://www.anthropic.com/claude-code) and related agentic-coding tools — slash commands, streaming tool use, context compaction — but extends the model with three things generic chat assistants don't have:

1. **Background isolated work** — spawn `--vex` tasks that run in their own git worktree, owned by a background daemon, so your chat stays responsive and tasks survive closing the TUI.
2. **A 7-phase pipeline** — Research → Ideation → Plan → Draft → Review → Implement → Docs, with an autonomous planner that picks the minimal subset per task.
3. **A live web dashboard** — kill/retry/inspect running tasks from your browser while chat keeps going.

## Table of contents

- [Quick start](#quick-start)
- [The TUI at a glance](#the-tui-at-a-glance)
- [Core features](#core-features)
- [Background tasks (--vex)](#background-tasks---vex)
- [The dashboard](#the-dashboard)
- [Slash commands](#slash-commands)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Multi-provider LLM support](#multi-provider-llm-support)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Development](#development)
- [License](#license)

## Quick start

```bash
# Clone and build
git clone https://github.com/d3vx-com/d3vx.git
cd d3vx
cargo build --release

# Set your API key (any supported provider works)
export ANTHROPIC_API_KEY="sk-ant-..."

# Launch
cargo run
```

First run: d3vx detects a missing config and offers an interactive setup wizard. Skip it with `n` if you only need the env var.

That's it. The background daemon auto-starts on launch so your vex tasks survive TUI exit. The web dashboard starts at `http://127.0.0.1:9876` — type `/dashboard` to open it.

## The TUI at a glance

```
┌───────────────────────────────────────────────────────────────────────┐
│  chat view / messages                                                 │
│                                                                       │
│  (or /board kanban, /list task list, /agents monitor, ...)            │
│                                                                       │
├───────────────────────────────────────────────────────────────────────┤
│  ▶  Type a message...                                                 │
│    › /board kanban  /list tasks  /dashboard web  /vex bg  ? all       │
├───────────────────────────────────────────────────────────────────────┤
│  ○ chat · claude-opus-4-7 · $0.03    dash ● :9876 · daemon ● · 0 bg   │
└───────────────────────────────────────────────────────────────────────┘
```

**Four persistent discovery surfaces** keep the product self-documenting:

- **Bottom status strip** — ambient state: current mode, model, cost, dashboard/daemon/bg-task indicators. When something's off (missing API key, daemon down), a warning lights up here.
- **Ghost hints above the prompt** — when the input is empty, the four most useful slash commands are shown dim. They vanish on first keystroke.
- **Live slash palette** — type `/` and a filtered dropdown of every command appears above the prompt. Arrow keys + Tab + Enter.
- **`/help`** — grouped by category (Discovery, Modes & views, Agents & tasks, Session, Git & content, Setup) with a keyboard-shortcut cheatsheet.

## Core features

### Autonomous phase selection

A planner decides whether your request is a trivial question, a single-step change, or something that needs a full pipeline — writes its decision to `.d3vx/plans/<id>.md` as a checkbox-driven markdown file. Plans survive crashes and can be resumed from the first unchecked section.

- `/plans` lists every plan with phase progress (`2/5 phases done`)
- Plans indicator on the status strip when there's in-flight work
- One shared `advance_one_step` primitive drives phases for both chat and `--vex`

### Background isolated task execution (`--vex`)

```bash
# From the CLI
cargo run -- --vex "refactor the auth middleware to use tower layers"

# From inside the TUI
/vex refactor the auth middleware to use tower layers
/vex list          # show all running vex tasks
```

What happens:
1. A git worktree is provisioned on a fresh branch (`vex/refactor-the-auth-middleware`)
2. The orchestrator queues the task
3. The daemon picks it up and runs it to completion with its own agent loop
4. You stay in your current conversation — the vex task doesn't block you
5. The TUI can close and the task keeps going (because the daemon owns it)

### Parallel agent orchestration

- Main agent operates as a **coordinator** when parallel mode is enabled
- Child agents run as **bounded executors** — no recursive spawning, dependency-aware scheduling
- **Best-of-N**: generate multiple variants, select the winner with a tie-break agent
- **Doom-loop detection** stops agents that get stuck repeating themselves
- **Budget enforcement** (per-session and per-day) prevents runaway cost

### Task persistence & resume

Every task, session, and message is persisted in SQLite (`~/.d3vx/d3vx.db`, WAL mode). Crash recovery on startup re-enqueues interrupted work. The `--resume` flag and `/resume` command pick up where you left off.

## Background tasks (`--vex`)

The `--vex` flag turns any task into a background job. Combined with the auto-started daemon, a `/vex "do X"` is **actually background**:

- Works in an isolated git worktree (never touches your working tree)
- Survives TUI close (the daemon owns the dispatch loop)
- Supports policy flags: `--review` (human approval before merge), `--merge` (auto-merge on green), `--docs` (generate documentation)
- Monitored via `/vex list`, the dashboard, or the status strip

## The dashboard

Auto-starts at `http://127.0.0.1:9876` (localhost only). React SPA + Axum backend + Server-Sent Events.

**What it shows:**
- Sortable task table: id, title, state, phase, cost, duration, branch, created time
- Per-task detail panel with full metadata and tool-execution history
- Real-time updates via SSE — new tasks and state changes push instantly
- Cost tracking with budget enforcement

**Interactive:**
- Kill any running task
- Retry any failed task
- Filter and search by state/phase

Open it from the TUI with `/dashboard` — the browser opens automatically and a toast shows the URL for copy/paste.

## Slash commands

The full list is reachable by typing `/` in the prompt (live palette) or via `/help`. Highlights:

| Category | Commands |
|---|---|
| **Discovery** | `/help`, `/dashboard`, `/daemon`, `/plans` |
| **Modes & views** | `/board`, `/list`, `/agents`, `/mode`, `/model`, `/verbose`, `/power`, `/vibe`, `/plan` |
| **Agents & tasks** | `/vex`, `/spawn`, `/thinking` |
| **Session** | `/clear`, `/compact`, `/status`, `/cost`, `/undo`, `/resume`, `/export` |
| **Git & content** | `/commit`, `/pr`, `/expand`, `/image` |
| **Setup** | `/setup`, `/doctor`, `/init`, `/pricing`, `/quit` |

## Keyboard shortcuts

**Chat:** `Enter` send · `\ + Enter` newline · `↑/↓` history · `Esc` stop / close · `Ctrl+C` interrupt (twice to quit)

**Slash palette:** `/` open · `↑/↓` navigate · `Tab` complete · `Enter` accept-and-run

**Views:** `Ctrl+1..4` right-pane tabs · `Ctrl+L` left sidebar · `Ctrl+W` detail drawer · `Ctrl+O` expand tool output · `Ctrl+F` cycle focus mode · `?` quick help

## Multi-provider LLM support

| Provider | Value | Key env var |
|---|---|---|
| Anthropic (Claude) | `anthropic` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai` | `OPENAI_API_KEY` |
| Groq | `groq` | `GROQ_API_KEY` |
| xAI (Grok) | `xai` | `XAI_API_KEY` |
| Mistral | `mistral` | `MISTRAL_API_KEY` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| Ollama (local) | `ollama` | *none* |

Switch provider/model with `/model <name>` or `/mode <chat|build|plan|docs|test|review>` (which can auto-route).

## Configuration

```
~/.d3vx/config.yml       # global
.d3vx/config.yml         # per-project override
.d3vx/project.md         # project context for the agent
.d3vx/plans/*.md         # autonomous plan files
~/.d3vx/d3vx.db          # SQLite (tasks, sessions, tools)
~/.d3vx/daemon-status.json   # live daemon heartbeat
```

**Budget enforcement example:**

```yaml
budget:
  per_session: 5.00
  per_day: 50.00
  warn_at: 0.8
  pause_at: 1.0
  enabled: true
```

**Model Context Protocol (MCP) servers:**

```yaml
mcp:
  servers:
    sqlite:
      command: "npx"
      args: ["-y", "@modelcontextprotocol/server-sqlite", "--db", "/path/to.db"]
```

## Architecture

Top-level modules (all in `src/`):

| Module | Owns |
|---|---|
| `agent/` | Agent loop, conversation, context compaction, sub-agent orchestration |
| `app/` | TUI state machine, slash commands, keyboard, rendering |
| `cli/` | Clap argument parsing, subcommands (`daemon`, `setup`, `doctor`, `vex`) |
| `pipeline/` | 7-phase engine, task queue, worker pool, GitHub integration, dashboard |
| `planner/` | Autonomous phase selection + markdown plan files |
| `providers/` | LLM provider abstractions (Anthropic SSE + OpenAI-compatible) |
| `store/` | SQLite persistence (tasks, sessions, messages, workspaces) |
| `tools/` | 40+ built-in tools (Bash, Read, Write, Edit, Glob, Grep, MCP, ...) |
| `mcp/` | Model Context Protocol client |
| `lsp/` | Language Server Protocol integration |

## Development

```bash
cargo build                                    # debug
cargo build --release                          # optimized
cargo test --lib                               # full test suite (2400+ tests)
cargo clippy --lib -- -D warnings              # lint
cargo fmt -- --check                           # format check
```

Project guidelines in [CLAUDE.md](CLAUDE.md) and [CONTRIBUTING.md](CONTRIBUTING.md). Files stay under ~300 lines; tests live in `*_tests.rs` siblings, never inline `mod tests {}`.

## Acknowledgements

d3vx stands on the shoulders of the broader terminal-AI and agentic-coding community. The product model — slash commands, streaming tool use, context compaction, isolated worktrees — draws directly from [Claude Code](https://www.anthropic.com/claude-code), [Anthropic's Agent SDK](https://docs.anthropic.com/en/api/agent-sdk), and the open source agentic ecosystem. The 7-phase pipeline borrows from Spec-Driven Development patterns.

Built on [ratatui](https://github.com/ratatui/ratatui), [tokio](https://tokio.rs/), [axum](https://github.com/tokio-rs/axum), [tree-sitter](https://tree-sitter.github.io/), and [git2](https://github.com/rust-lang/git2-rs).

## License

Licensed under the [Apache License, Version 2.0](LICENSE). Commercial use, modification, and redistribution are all permitted; see the LICENSE file for the full terms, including the patent grant.
