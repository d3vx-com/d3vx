//! File and folder icons module
//!
//! Provides file and folder related icons using unicode symbols (Box Drawing + Geometric Shapes).

/// ▣ Filled square (folder closed)
pub const fn folder() -> &'static str {
    "▣"
}

/// ▢ Square outline (folder open)
pub const fn folder_open() -> &'static str {
    "▢"
}

/// ▤ Horizontal lines (document)
pub const fn file() -> &'static str {
    "▤"
}

/// ▥ Vertical lines (document)
pub const fn file_text() -> &'static str {
    "▥"
}

/// ⊞ Four squares (clipboard/box)
pub const fn clipboard() -> &'static str {
    "⊞"
}

/// ◈ Diamond with dot (locked)
pub const fn lock() -> &'static str {
    "◈"
}

/// ◇ Diamond outline (unlocked)
pub const fn lock_open() -> &'static str {
    "◇"
}

/// ⌧ Control key (key icon)
pub const fn key() -> &'static str {
    "⌧"
}

/// ⊛ Circled asterisk (package)
pub const fn package() -> &'static str {
    "⊛"
}

/// ⊠ Box with X (archive)
pub const fn archive() -> &'static str {
    "⊠"
}

/// ⊡ Box with dot (inbox)
pub const fn inbox() -> &'static str {
    "⊡"
}

/// ☰ Hamburger menu (list)
pub const fn list() -> &'static str {
    "☰"
}

/// ⊟ Box with plus (new file)
pub const fn new_file() -> &'static str {
    "⊟"
}

/// ⊠ Box with X (remove)
pub const fn remove() -> &'static str {
    "⊠"
}

/// ◻ Square outline
pub const fn square() -> &'static str {
    "◻"
}

/// ◼ Filled square
pub const fn square_filled() -> &'static str {
    "◼"
}
