//! Symbols - Unicode symbols for the TUI
//!
//! Shared symbols for consistent display across components,
//! matching src/tui/utils/symbols.ts

// ────────────────────────────────────────────────────────────
// AI Indicators
// ────────────────────────────────────────────────────────────

pub const AI_INDICATOR: &str = "❯";
pub const AI_INDICATOR_ALT: &str = "⬡";
pub const USER_INDICATOR: &str = "❯";
pub const SHELL_INDICATOR: &str = "$";

// ────────────────────────────────────────────────────────────
// Status Icons
// ────────────────────────────────────────────────────────────

pub const STATUS: StatusSymbols = StatusSymbols {
    success: "✓",
    error: "✕",
    pending: "◌",
    running: "◌",
    warning: "⚠",
    info: "ℹ",
    blocked: "⊘",
    skipped: "○",
};

pub struct StatusSymbols {
    pub success: &'static str,
    pub error: &'static str,
    pub pending: &'static str,
    pub running: &'static str,
    pub warning: &'static str,
    pub info: &'static str,
    pub blocked: &'static str,
    pub skipped: &'static str,
}

// ────────────────────────────────────────────────────────────
// UI Elements
// ────────────────────────────────────────────────────────────

pub const UI: UiSymbols = UiSymbols {
    line_major: "━",
    line_vertical: "│",
    corner_tl: "┌",
    corner_tr: "┐",
    corner_bl: "└",
    corner_br: "┘",
    arrow_right: "→",
    arrow_down: "↓",
    ellipsis: "…",
    separator_dot: "·",
    section: "▸",
    brand: "✻",
    tip: "✦",
    lightning: "⌁",
};

pub struct UiSymbols {
    pub line_major: &'static str,
    pub line_vertical: &'static str,
    pub corner_tl: &'static str,
    pub corner_tr: &'static str,
    pub corner_bl: &'static str,
    pub corner_br: &'static str,
    pub arrow_right: &'static str,
    pub arrow_down: &'static str,
    pub ellipsis: &'static str,
    pub separator_dot: &'static str,
    pub section: &'static str,
    pub brand: &'static str,
    pub tip: &'static str,
    pub lightning: &'static str,
}

pub const POINTER: &str = "❯";
pub const ARROW_UP: &str = "↑";
pub const ARROW_DOWN: &str = "↓";
pub const ARROW_RIGHT: &str = "→";
pub const ARROW_LEFT: &str = "←";
pub const ELLIPSIS: &str = "…";
pub const BULLET: &str = "•";
pub const CHECK: &str = "✓";
pub const CROSS: &str = "✕";
pub const GEAR: &str = "⬢";
pub const LOCK: &str = "⊘";
pub const UNLOCK: &str = "○";

// ────────────────────────────────────────────────────────────
// Borders
// ────────────────────────────────────────────────────────────

pub const BORDER: BorderSymbols = BorderSymbols {
    horizontal: "─",
    vertical: "│",
    top_left: "┌",
    top_right: "┐",
    bottom_left: "└",
    bottom_right: "┘",
};

pub struct BorderSymbols {
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
}

pub const BORDER_ROUNDED: RoundedBorderSymbols = RoundedBorderSymbols {
    horizontal: "─",
    vertical: "│",
    top_left: "╭",
    top_right: "╮",
    bottom_left: "╰",
    bottom_right: "╯",
};

pub struct RoundedBorderSymbols {
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
}

// ────────────────────────────────────────────────────────────
// Diff
// ────────────────────────────────────────────────────────────

pub const DIFF_ADDED: &str = "+";
pub const DIFF_REMOVED: &str = "-";
pub const DIFF_UNCHANGED: &str = " ";

// ────────────────────────────────────────────────────────────
// Spinners
// ────────────────────────────────────────────────────────────

/// Simple spinner frames
pub const SPINNER_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Elegant spinner frames (matching Claude Code / Main)
pub const SPINNER_ELEGANT: &[&str] = &["·", "✢", "✳", "✶", "✻", "✽"];

/// Get spinner frames for the current platform
pub fn get_spinner_frames() -> &'static [&'static str] {
    if std::env::var("TERM").ok().as_deref() == Some("xterm-ghostty") {
        &["·", "✢", "✳", "✶", "✻", "*"]
    } else if cfg!(target_os = "macos") {
        SPINNER_ELEGANT
    } else {
        &["·", "✢", "*", "✶", "✻", "✽"]
    }
}

// ────────────────────────────────────────────────────────────
// Thinking
// ────────────────────────────────────────────────────────────

pub const THINKING_DOTS: &str = "⋯";
pub const THINKING_BRAIN: &str = "⬡";

// ────────────────────────────────────────────────────────────
// Tool Categories
// ────────────────────────────────────────────────────────────

/// Get icon for a tool name (using unicode symbols)
pub fn get_tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        // File tools
        "ReadTool" | "Read" => "▤",
        "WriteTool" | "Write" => "✍",
        "EditTool" | "Edit" | "MultiEditTool" => "✎",

        // Search tools
        "GrepTool" | "Grep" => "⌕",
        "GlobTool" | "Glob" => "◫",

        // Execute tools
        "BashTool" | "Bash" => "↯",
        "Task" => "☰",

        // Network tools
        "WebSearchTool" | "webSearchTool" => "⌬",
        "WebFetchTool" | "WebFetch" => "∞",

        // Special tools
        "ThinkTool" | "Think" => "⬡",
        "QuestionTool" | "Question" => "?",
        "TodoWriteTool" | "TodoWrite" => "✍",

        // Default
        _ => "▸",
    }
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_symbols() {
        assert_eq!(STATUS.success, "✓");
        assert_eq!(STATUS.error, "✕");
        assert_eq!(STATUS.pending, "◌");
    }

    #[test]
    fn test_spinner_frames() {
        let frames = get_spinner_frames();
        assert!(!frames.is_empty());
        assert!(frames.len() >= 5);
    }

    #[test]
    fn test_tool_icons() {
        assert_eq!(get_tool_icon("ReadTool"), "▤");
        assert_eq!(get_tool_icon("BashTool"), "↯");
    }
}
