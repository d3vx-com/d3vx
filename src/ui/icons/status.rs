//! Status icons module
//!
//! Provides common status indicators using unicode symbols.

/// ✓ Check mark (success)
pub const fn check() -> &'static str {
    "✓"
}

/// ✗ Cross mark (error)
pub const fn x() -> &'static str {
    "✗"
}

/// ○ Circle (neutral/pending)
pub const fn circle() -> &'static str {
    "○"
}

/// ◉ Filled circle (active)
pub const fn circle_filled() -> &'static str {
    "◉"
}

/// ● Dot (small indicator)
pub const fn dot() -> &'static str {
    "●"
}

/// ⚠ Warning
pub const fn warning() -> &'static str {
    "⚠"
}

/// ℹ Info
pub const fn info() -> &'static str {
    "ℹ"
}

/// ✔ Heavy check mark
pub const fn check_bold() -> &'static str {
    "✔"
}

/// ✘ Heavy X mark
pub const fn x_bold() -> &'static str {
    "✘"
}

/// ◌ Dashed circle
pub const fn circle_dashed() -> &'static str {
    "◌"
}

/// ⟳ Reload/refresh
pub const fn reload() -> &'static str {
    "⟳"
}

/// ⟲ Undo arrow
pub const fn undo() -> &'static str {
    "⟲"
}

/// ↻ Redo arrow
pub const fn redo() -> &'static str {
    "↻"
}

/// ⊛ Circled asterisk
pub const fn status_active() -> &'static str {
    "⊛"
}

/// ⊘ Circled slash
pub const fn status_blocked() -> &'static str {
    "⊘"
}

/// ⊚ Circled white slash
pub const fn status_off() -> &'static str {
    "⊚"
}

/// ◎ Bullseye
pub const fn target() -> &'static str {
    "◎"
}
