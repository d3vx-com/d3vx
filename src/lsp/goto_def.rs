//! Go-to Definition Provider
//!
//! Provides go-to-definition and find-references functionality.

use lsp_types::{Location, Position, Range};
use std::path::Path;

pub struct GotoProvider;

impl GotoProvider {
    pub fn new() -> Self {
        Self
    }

    /// Format a location for display
    pub fn format_location(&self, location: &Location) -> String {
        let uri_path = location.uri.path().as_str();
        let path = Path::new(uri_path);
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| uri_path.to_string());

        let line = location.range.start.line + 1;
        let col = location.range.start.character + 1;

        format!("{}:{}:{}", file, line, col)
    }

    /// Format multiple locations
    pub fn format_locations(&self, locations: &[Location]) -> Vec<String> {
        locations.iter().map(|l| self.format_location(l)).collect()
    }

    /// Create a human-readable summary
    pub fn format_summary(&self, locations: &[Location], symbol_name: &str) -> String {
        if locations.is_empty() {
            return format!("No references found for '{}'", symbol_name);
        }

        let unique_files: std::collections::HashSet<_> =
            locations.iter().map(|l| l.uri.path().to_string()).collect();

        format!(
            "'{}' found in {} location(s) across {} file(s)",
            symbol_name,
            locations.len(),
            unique_files.len()
        )
    }

    /// Get the file path from a location
    pub fn get_file(&self, location: &Location) -> String {
        location.uri.path().to_string()
    }

    /// Get the line number from a location
    pub fn get_line(&self, location: &Location) -> u32 {
        location.range.start.line + 1
    }

    /// Check if location is in the same file
    pub fn is_same_file(&self, loc1: &Location, loc2: &Location) -> bool {
        loc1.uri == loc2.uri
    }
}
