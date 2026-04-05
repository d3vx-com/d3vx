//! Code Map Types
//!
//! Core data structures and constants for the code map system.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A file that has been scored against a query.
#[derive(Debug, Clone)]
pub struct ScoredFile {
    pub path: PathBuf,
    pub score: f64,
    pub matched_terms: Vec<String>,
}

/// Metadata and extracted symbols for a single source file.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub line_count: usize,
    pub identifiers: Vec<String>,
    pub calls: Vec<String>,
    pub defined_symbols: Vec<String>,
    pub score: f64,
}

/// A map of all source files in a project with their symbol information.
#[derive(Debug, Clone)]
pub struct CodeMap {
    pub files: HashMap<PathBuf, FileEntry>,
}

// -- Constants --------------------------------------------------------------

pub(crate) const SUPPORTED_EXTENSIONS: &[&str] = &["rs", "js", "ts", "tsx", "py"];
pub(crate) const KEYWORDS: &[&str] = &[
    "let",
    "const",
    "var",
    "fn",
    "func",
    "function",
    "def",
    "class",
    "struct",
    "enum",
    "impl",
    "trait",
    "interface",
    "type",
    "pub",
    "public",
    "private",
    "protected",
    "static",
    "self",
    "super",
    "return",
    "break",
    "continue",
    "async",
    "await",
    "use",
    "import",
    "export",
    "from",
    "mod",
    "crate",
    "if",
    "else",
    "for",
    "while",
    "loop",
    "match",
    "switch",
    "case",
    "true",
    "false",
    "some",
    "none",
    "null",
    "undefined",
    "new",
    "mut",
    "ref",
    "move",
    "where",
    "with",
    "the",
    "and",
    "not",
    "has",
    "get",
    "set",
    "put",
    "all",
    "into",
];
pub(crate) const SKIP_DIRS: &[&str] = &["target", "node_modules", "vendor", "dist", "build"];

pub(crate) fn is_supported_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e))
        .unwrap_or(false)
}

pub(crate) fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}
