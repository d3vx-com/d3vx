# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when operating in this repository.

## Project Overview

**d3vx** is a Rust-based autonomous software engineering CLI — a terminal application that uses AI agents to perform software engineering tasks. Key differentiators from generic AI coding tools include background isolated agents (via git worktrees), a 7-phase pipeline, parallel orchestration, and task persistence in SQLite.

## Essential Commands

### Build & Run

```bash
# Build (debug)
cargo build

# Build (optimized release)
cargo build --release

# Run the TUI
cargo run

# Run with provider configured
export ANTHROPIC_API_KEY="sk-ant-..."
cargo run

# Run specific view
cargo run -- --ui kanban
cargo run -- --ui list

# Setup wizard for first-time configuration
cargo run -- setup

# One-shot query (non-interactive)
cargo run -- "explain the auth module"
```

### Tests

```bash
# Run all tests
cargo test

# Run library tests only
cargo test --lib

# Run integration tests
cargo test --test '*'

# Run a single test
cargo test test_name_here

# Run tests for a specific module
cargo test agent::
cargo test pipeline::
cargo test tools::
```

### Code Quality

```bash
# Lint (must pass before PR)
cargo clippy --all-targets -- -D warnings

# Format check
cargo fmt -- --check

# Type checking + compilation verification
cargo check --tests
```

### Custom Profiles

- `cargo build --profile fast-release` — Faster compilation, good optimization (no LTO, 16 codegen units)
- `cargo build --release` — Full optimization (LTO, 1 codegen unit, stripped)

## Architecture

### Top-Level Module Structure

The crate is organized into these top-level modules:

| Module | Purpose |
|--------|---------|
| `agent/` | Core agent loop, conversation management, context compaction, doom-loop detection, sub-agent orchestration, specialist agents |
| `app/` | TUI application state machine — main event loop, UI state, input handling, slash commands |
| `cli/` | CLI argument parsing with clap (derive mode) — subcommands for daemon, spawn, pipeline, etc. |
| `config/` | Configuration loading from YAML — global (`~/.d3vx/config.yml`) and per-project (`.d3vx/config.yml`) |
| `pipeline/` | 6-phase pipeline engine — Research → Plan → Draft → Review → Implement → Docs, with task queue, worker pool, and GitHub integration |
| `tools/` | 40+ built-in tools — Bash, Read, Write, Edit, Glob, Grep, Skill, MCP, web fetch/search, and more |
| `providers/` | LLM provider abstractions — Anthropic (custom SSE), OpenAI-compatible (Groq, xAI, Mistral, DeepSeek, OpenRouter, Ollama) |
| `store/` | SQLite persistence layer — sessions, tasks, messages, workspaces, workers, events |
| `services/` | Background services — daemon management, symbol extraction, memory search, hooks |
| `ipc/` | Inter-process communication — SDK and transport for daemon/TUI communication |
| `mcp/` | Model Context Protocol client — MCP server management and resource access |
| `lsp/` | Language Server Protocol integration — diagnostics, completion, goto-definition |
| `ui/` | Terminal UI widgets — ratatui/crossterm components, themes, icons |
| `skills/` | On-demand skill loading from SKILL.md files |
| `notifications/` | Notification system — Telegram integration |
| `recovery/` | Crash recovery and session restoration |

### Key Architectural Concepts

1. **TUI App State Machine** — The `App` struct lives in `app/` and coordinates IPC communication with the agent, UI rendering via ratatui, and session management.

2. **Agent Loop** — `agent/agent_loop` runs the conversation → tool execution → response cycle. The `tool_coordinator` manages tool registration and execution. Context compaction auto-summarizes when approaching token limits. Doom-loop detection prevents infinite patterns.

3. **Pipeline Engine** — `pipeline/` provides a 6-phase autonomous execution system. Tasks flow through Research → Plan → Draft → Review → Implement → Docs phases, each with dedicated handlers. Uses a task queue with priority, a worker pool, and dependency-aware scheduling.

4. **Background Agents (`--vex`)** — Tasks can be isolated in git worktrees. The `pipeline/orchestrator` is the central authority, managing task creation, queue, and execution. Child agents run bounded without recursive spawning.

5. **Persistence** — `store/` provides SQLite-backed storage with migrations. Sessions, tasks, messages, workspaces, and workers are all persisted.

6. **Multi-Provider** — LLM providers are abstracted behind the `Provider` trait. Anthropic uses a custom SSE protocol; other providers go through the OpenAI-compatible adapter.

### Configuration Files

- `~/.d3vx/config.yml` — Global configuration (provider, model, MCP servers, etc.)
- `.d3vx/config.yml` — Per-project overrides
- `.d3vx/project.md` — Project context and description
- `.env` — API keys (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)

### Database

SQLite database at `~/.d3vx/d3vx.db` (in memory mode supported for testing). Uses WAL mode for concurrent read/write. Migrations in `store/migrations/`.

## Code Conventions (from CONTRIBUTING.md)

- **Error handling** — Use `Result<T, E>` and `?` operator, never `unwrap()` in production
- **No `unsafe`** — Avoid unless absolutely necessary
- **No `println!` / `eprintln!`** — Use `tracing` macros for logging
- **Module naming** — `kebab-case` files, `snake_case` modules, `PascalCase` types
- **Testing** — Every new module needs `#[cfg(test)] mod tests`
- **File size** — Files generally stay under 300 lines; decompose into submodules when they grow
