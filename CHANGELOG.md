# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- ToolState extraction for tool execution state management (GitHub issue #37 Phase F)
  - Created `src/app/state/tool_state.rs` with ToolState struct containing 6 tool-related fields
  - Includes: tool_coordinator, executing_tools, recent_tools, expanded_tool_calls, activity_tools_expanded, standalone_tools_enabled
  - Added constructor, helper methods, and unit tests
  - Integrated ToolState into App struct with `pub tools: ToolState` field
  - Removed 6 legacy tool fields from App struct
  - Updated all access patterns from self.X to self.tools.X throughout codebase
- LayoutState extraction for layout tracking state management (GitHub issue #37 Phase G)
  - Created `src/app/state/layout_state.rs` with LayoutState struct containing 14 layout fields
  - Includes: left_sidebar_workspace_rows, sidebar_agent_rows, chat_agent_y_positions, chat_total_lines
  - Includes: activity_agent_y_positions, activity_diff_y_positions
  - Includes: last_left_sidebar_rect, last_right_sidebar_rect, last_input_rect, last_chat_rect
  - Includes: last_activity_rect, last_agent_detail_rect, last_tab_bar_rect, last_mode_bar_rect
  - Added Default implementation, helper methods, and unit tests
  - Integrated LayoutState into App struct with `pub layout: LayoutState` field
  - Removed 14 legacy layout fields from App struct
  - Updated all access patterns from self.X to self.layout.X throughout codebase
- AgentState extraction and integration for agent-related state management (GitHub issue #37 Phase E)
  - Created `src/app/state/agent_state.rs` with AgentState struct containing 15 agent-related fields
  - Includes: is_connected, workspace_agents, pending_agent_receivers, agent_loop, streaming_message
  - Includes: inline_agents, selected_inline_agent, parallel_agents_enabled
  - Includes: parallel_batches, spawn_parallel_receiver, pending_agent_queue, running_parallel_agents, active_parallel_batches
  - Added Default implementation and helper methods (has_active_agent, inline_agent_count, clear, parallel_batches_metadata)
  - Added comprehensive unit tests
  - Integrated AgentState into App struct with `pub agents: AgentState` field
  - Removed 15 legacy agent fields from App struct
  - Updated all access patterns from self.X to self.agents.X throughout codebase
  - All 62 state machine tests pass
- SessionState extraction for session-related state management (GitHub issue #37 Phase D)
  - Created `src/app/state/session_state.rs` with SessionState struct containing 13 session-related fields
  - Migrated: session_id, home_session_id, stream_out, messages, message_queue, pending_images
  - Migrated: permission_request, init_hint, thinking, thinking_start, token_usage
  - Migrated: session_cost, formatted_cost
  - Added Default implementation and helper methods (add_message, clear, queue_message, start_thinking, stop_thinking)
  - Added comprehensive unit tests for SessionState
  - Updated all access patterns from self.X to self.session.X throughout codebase
  - All 782 tests pass
- UIState foundation for modular UI management (GitHub issue #37 Phase A)
  - Created `src/app/state/ui_state.rs` with UIState struct containing ~30 UI-related fields extracted from App
  - Added Default implementation and comprehensive unit tests
  - Established module structure for future UI component extractions
- UIState integration into application architecture (GitHub issue #37 Phase B)
  - Added `pub ui: UIState` field to App struct for centralized UI state management
  - Updated test file to include UIState field ensuring test coverage
  - All 64 state machine tests pass validating UIState integration
- Complete UIState field migration (GitHub issue #37 Phase C)
  - Migrated mode, plan_mode, verbose, theme, power_mode, show_welcome fields
  - Migrated scroll state: scroll_offset, activity_scroll_offset, activity_content_lines, selected_agent_output_scroll, selected_agent_output_lines, max_scroll, help_scroll
  - Migrated input state: input_buffer, cursor_position, multiline_pending, focus_mode
  - Migrated sidebar state: sidebar_width, agent_monitor_pinned, right_sidebar_visible, selected_right_pane_tab
  - Migrated history state: input_history, history_index, history_prefix
  - Migrated mention state: mention_suggestions, mention_selected
  - Migrated escape tracking: escape_count, last_escape_time
  - Migrated command palette: command_palette_filter, command_palette_selected
  - Migrated layout tracking: last_left_sidebar_rect, last_right_sidebar_rect, last_input_rect, last_chat_rect, etc.
  - Migrated row tracking: left_sidebar_workspace_rows, sidebar_agent_rows, chat_agent_y_positions, etc.
  - Updated all access patterns from self.field to self.ui.field throughout codebase
  - Removed 19 legacy fields from App struct
  - All 64 state machine tests pass
- Clone reduction in hot paths (GitHub issue #38)
  - Refactored reaction/mod.rs handler methods to return HandlerDecision struct instead of ReactionResult, eliminating ~22 event.clone() calls per handler invocation
  - Partial config read in agent_loop execute_tools (only extracts 3 needed fields instead of cloning entire AgentConfig)
  - Replaced child.result.clone() with as_deref() pattern in legacy.rs evaluate_parallel_child
  - Replaced clone+clear with std::mem::take for code blocks and thinking state in legacy.rs

### Changed
- Reduced App struct complexity by ~30 UI-related fields moved to UIState
- Reduced App struct complexity by 13 session-related fields moved to SessionState
- Reduced App struct complexity by 15 agent-related fields moved to AgentState
- Reduced App struct complexity by 6 tool execution fields moved to ToolState
- Reduced App struct complexity by 14 layout tracking fields moved to LayoutState
- Business data fields (workspaces, git_changes, etc.) remain in App
- Reduced clone allocations in pipeline reaction engine (~22 fewer event clones per reaction cycle)
- Reduced clone allocations in agent loop (partial config read, eliminated redundant total_usage clones)

### Fixed
- Consolidated duplicate `braille_frame()` function and `BRAILLE_FRAMES` constant from multiple files into single source in `src/app/ui/helpers.rs` (GitHub issue #35)
- Updated imports in `inline_agents.rs` and `ui_legacy.rs` to use consolidated implementation
- Removed duplicate `status_icon()` method from `legacy.rs` and updated import to use consolidated implementation from `extraction` module (GitHub issue #36)

## [0.1.0] - 2026-02-27

### Added
- Initial project scaffold
- Build system with tsup
- TypeScript configuration
- ESLint and Prettier configuration
- Vitest test framework
- Basic CLI with version command
