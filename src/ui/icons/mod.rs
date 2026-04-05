//! Icon Module - Scalable icon system for the TUI
//!
//! Provides a curated set of icons organized by category.
//! Icons use unicode symbols for terminal compatibility.

pub mod arrows;
pub mod dev;
pub mod files;
pub mod git;
pub mod status;
pub mod ui;
pub mod weather;

pub use arrows::{arrow_down, arrow_left, arrow_right, arrow_up, chevron_left, chevron_right};
pub use dev::{box_bullet, code, database, layers, terminal};
pub use files::{file, file_text, folder, folder_open, lock};
pub use git::{branch, commit, compare, diff};
pub use status::{check, circle, info, warning, x};
pub use ui::{close, copy, edit, minus, plus, search, settings};
pub use weather::{bookmark, calendar, clock, star, timer};

#[cfg(test)]
mod tests {
    #[test]
    fn test_status_icons() {
        assert_eq!(super::check(), "✓");
        assert_eq!(super::x(), "✗");
        assert_eq!(super::circle(), "○");
    }

    #[test]
    fn test_file_icons() {
        assert_eq!(super::folder(), "▣");
        assert_eq!(super::file(), "▤");
    }

    #[test]
    fn test_git_icons() {
        assert_eq!(super::branch(), "⑂");
        assert_eq!(super::commit(), "●");
    }
}
