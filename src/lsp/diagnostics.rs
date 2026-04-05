//! Diagnostics Provider
//!
//! Processes and formats LSP diagnostics for display.

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

pub struct DiagnosticManager {
    debounce_ms: u64,
    cache: HashMap<String, Vec<FormattedDiagnostic>>,
}

impl Default for DiagnosticManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticManager {
    pub fn new() -> Self {
        Self {
            debounce_ms: 300,
            cache: HashMap::new(),
        }
    }

    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Format diagnostics for a file
    pub fn format_diagnostics(
        &self,
        path: &Path,
        diagnostics: &[Diagnostic],
    ) -> Vec<FormattedDiagnostic> {
        diagnostics
            .iter()
            .map(|d| FormattedDiagnostic {
                file: path.to_string_lossy().to_string(),
                severity: format_severity(d.severity),
                line: d.range.start.line + 1,
                column: d.range.start.character + 1,
                code: d
                    .code
                    .as_ref()
                    .map(|c| match c {
                        lsp_types::NumberOrString::Number(n) => n.to_string(),
                        lsp_types::NumberOrString::String(s) => s.clone(),
                    })
                    .unwrap_or_default(),
                message: d.message.clone(),
                source: d.source.clone().unwrap_or_else(|| "LSP".to_string()),
            })
            .collect()
    }

    /// Convert to pretty string format
    pub fn to_pretty(&self, diagnostics: &[FormattedDiagnostic]) -> String {
        if diagnostics.is_empty() {
            return "✓ No issues found".to_string();
        }

        let mut output = String::new();

        // Group by file
        let by_file: HashMap<_, _> = diagnostics.iter().fold(HashMap::new(), |mut acc, d| {
            acc.entry(&d.file).or_insert_with(Vec::new).push(d);
            acc
        });

        for (file, diags) in by_file {
            output.push_str(&format!("{}:\n", file));
            for diag in diags {
                output.push_str(&format!(
                    "  {}:{}:{} [{}] {}\n",
                    diag.line, diag.column, diag.code, diag.severity, diag.message
                ));
            }
            output.push('\n');
        }

        output.trim().to_string()
    }

    /// Get summary statistics
    pub fn summarize(&self, diagnostics: &[FormattedDiagnostic]) -> DiagnosticSummary {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        let mut hints = 0;

        for d in diagnostics {
            match d.severity.to_lowercase().as_str() {
                "error" | "critical" => errors += 1,
                "warning" => warnings += 1,
                "information" | "info" => infos += 1,
                "hint" => hints += 1,
                _ => {}
            }
        }

        DiagnosticSummary {
            total: diagnostics.len(),
            errors,
            warnings,
            infos,
            hints,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormattedDiagnostic {
    pub file: String,
    pub severity: String,
    pub line: u32,
    pub column: u32,
    pub code: String,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

fn format_severity(severity: Option<DiagnosticSeverity>) -> String {
    match severity {
        Some(DiagnosticSeverity::ERROR) => "error".to_string(),
        Some(DiagnosticSeverity::WARNING) => "warning".to_string(),
        Some(DiagnosticSeverity::INFORMATION) => "info".to_string(),
        Some(DiagnosticSeverity::HINT) => "hint".to_string(),
        _ => "unknown".to_string(),
    }
}
