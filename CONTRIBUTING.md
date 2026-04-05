# CONTRIBUTING to d3vx

Welcome! We are thrilled to have your contributions.

## Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/d3vx-com/d3vx.git
   cd d3vx
   ```

2. **Build the project:**
   ```bash
   cargo build
   ```

3. **Run the application:**
   ```bash
   cargo run
   ```

4. **Run tests:**
   ```bash
   cargo test --lib
   ```

5. **Code quality checks (must pass before submitting a PR):**
   ```bash
   cargo clippy --all-targets -- -D warnings
   cargo fmt -- --check
   cargo check --tests
   ```

## Project Structure

The main Rust TUI runtime lives in `src-tui/`. Key modules:

| Module | Purpose |
|--------|---------|
| `agent/` | Agent loop, conversation, doom loop detection, context compaction |
| `tools/` | 42 tools: Bash, Read, Write, Edit, Glob, Grep, Skill, MCP, and more |
| `providers/` | LLM adapters (Anthropic, OpenAI-compatible for others) |
| `pipeline/` | 7-phase pipeline (Research → Ideation → Plan → Draft → Review → Implement → Docs) |
| `ui/` | Terminal UI with ratatui, themes, keybindings |
| `mcp/` | Model Context Protocol client |
| `store/` | SQLite database layer for sessions, tasks, logs |
| `config/` | Configuration loading with YAML schema |

## Code Conventions

- **Strict error handling** — Use `Result<T, E>` and `?` operator, never `unwrap()` in production code
- **No `unsafe`** — Avoid unsafe code unless absolutely necessary
- **No `println!` / `eprintln!`** — Always use `tracing` macros
- **Module naming** — `kebab-case` files, `snake_case` modules, `PascalCase` types
- **Testing** — Every new module must have a `#[cfg(test)] mod tests` block
- **File size** — Production files generally stay under 300 lines; decompose into submodules when they grow

## Pull Requests

- Ensure `cargo clippy`, `cargo fmt -- --check`, and `cargo test` all pass
- Include tests for new functionality
- Keep commits focused and descriptive
