//! Git icons module
//!
//! Provides git-related icons using unicode symbols.

/// ⑂ Branch icon
pub const fn branch() -> &'static str {
    "⑂"
}

/// ● Commit dot
pub const fn commit() -> &'static str {
    "●"
}

/// ⇄ Double arrow (push/pull)
pub const fn push_pull() -> &'static str {
    "⇄"
}

/// ⇅ Up/down arrow
pub const fn fetch() -> &'static str {
    "⇅"
}

/// ⇃ Up arrow with tail
pub const fn push() -> &'static str {
    "⇃"
}

/// ⇂ Down arrow with tail
pub const fn pull() -> &'static str {
    "⇂"
}

/// ⊙ Circled dot (uncommitted)
pub const fn uncommitted() -> &'static str {
    "⊙"
}

/// ◉ Merged (circle filled)
pub const fn merged() -> &'static str {
    "◉"
}

/// ⇆ Left right arrows
pub const fn compare() -> &'static str {
    "⇆"
}

/// ↔ Horizontal arrows
pub const fn diff() -> &'static str {
    "↔"
}

/// ↕ Vertical arrows
pub const fn diff_vertical() -> &'static str {
    "↕"
}

/// ⊕ Circled plus (add)
pub const fn add() -> &'static str {
    "⊕"
}

/// ⊖ Circled minus (remove)
pub const fn remove() -> &'static str {
    "⊖"
}

/// ⊜ Circled equals
pub const fn modified() -> &'static str {
    "⊜"
}

/// ⚡ Lightning (stash)
pub const fn stash() -> &'static str {
    "⚡"
}

/// ⌂ House (home/root)
pub const fn home() -> &'static str {
    "⌂"
}

/// ⊗ Circled times (conflict)
pub const fn conflict() -> &'static str {
    "⊗"
}
