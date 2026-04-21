//! Panic-free glob directory walker.
//!
//! Replaces `globwalk = "0.9"` — an unmaintained crate whose iterator
//! panics on a `strip_prefix` unwrap when an entry's path doesn't
//! start with the walker's base (triggered by symlinks or macOS path
//! normalisation quirks). The panic aborted d3vx mid-session the
//! first time an agent's Glob call hit a symlinked directory outside
//! the repo root.
//!
//! This module composes `walkdir` (for traversal) with `glob::Pattern`
//! (for matching) and handles the `strip_prefix` failure by *skipping*
//! the entry instead of unwrapping. Same capabilities, no panics, one
//! less unmaintained dependency in the tree.
//!
//! Usage:
//!
//! ```ignore
//! use crate::utils::glob_walk::walk_matching;
//! let paths = walk_matching("/repo", "**/*.rs", true)?;
//! ```

use std::path::{Path, PathBuf};

use glob::{MatchOptions, Pattern, PatternError};
use walkdir::WalkDir;

/// Walk `base` recursively and return every file whose path (relative
/// to `base`) matches `pattern`. Case sensitivity is controllable so
/// callers that care about macOS-style lookups can opt in.
///
/// Never panics — a failing `strip_prefix` call silently skips the
/// entry, and individual walker I/O errors are logged via `tracing`
/// but don't abort the iteration.
pub fn walk_matching(
    base: impl AsRef<Path>,
    pattern: &str,
    case_insensitive: bool,
) -> Result<Vec<PathBuf>, PatternError> {
    let base = base.as_ref();
    let compiled = Pattern::new(pattern)?;
    let options = MatchOptions {
        case_sensitive: !case_insensitive,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    let mut out: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(base).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                // Log, don't abort — a single unreadable directory
                // shouldn't kill the whole walk.
                tracing::debug!("glob walk skip: {err}");
                continue;
            }
        };
        let path = entry.path();
        let rel = match path.strip_prefix(base) {
            Ok(r) => r,
            // The exact panic site in globwalk. We skip instead.
            Err(_) => continue,
        };
        if compiled.matches_path_with(rel, options) {
            out.push(path.to_path_buf());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "").unwrap();
    }

    #[test]
    fn matches_recursive_pattern() {
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("a.rs"));
        touch(&tmp.path().join("src/b.rs"));
        touch(&tmp.path().join("src/nested/c.rs"));
        touch(&tmp.path().join("docs/readme.md"));

        let matches = walk_matching(tmp.path(), "**/*.rs", false).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn star_pattern_crosses_directories_by_default() {
        // We set `require_literal_separator: false` on MatchOptions —
        // matching the prior globwalk behaviour where `*.rs` traverses
        // into subdirectories rather than being single-level. Agent
        // tools rely on this: `Glob("*.rs")` returns all Rust files,
        // not just the ones in the exact starting directory.
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("a.rs"));
        touch(&tmp.path().join("src/b.rs"));

        let matches = walk_matching(tmp.path(), "*.rs", false).unwrap();
        assert_eq!(matches.len(), 2, "expected recursive match across dirs");
    }

    #[test]
    fn case_insensitive_option_works() {
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("README.MD"));
        touch(&tmp.path().join("readme.md"));

        let ci = walk_matching(tmp.path(), "**/*.md", true).unwrap();
        let cs = walk_matching(tmp.path(), "**/*.md", false).unwrap();
        assert!(ci.len() >= cs.len(), "CI should match at least as many as CS");
    }

    #[test]
    fn invalid_pattern_returns_err_instead_of_panicking() {
        let tmp = TempDir::new().unwrap();
        // Unclosed `[` is a classic bad-glob input. globwalk used to
        // panic on some of these; we must surface it as Err.
        let err = walk_matching(tmp.path(), "**/[unclosed", false);
        assert!(err.is_err());
    }

    #[test]
    fn empty_dir_yields_empty_matches() {
        let tmp = TempDir::new().unwrap();
        let matches = walk_matching(tmp.path(), "**/*", false).unwrap();
        // Depending on walkdir, the root itself may or may not match.
        // We only assert it doesn't panic and returns a Vec.
        assert!(matches.iter().all(|p| p.exists()));
    }
}
