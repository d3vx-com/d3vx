//! Completion Provider
//!
//! Provides code completion suggestions from LSP.

use lsp_types::CompletionItem;
use std::collections::HashMap;

pub struct CompletionProvider {
    #[allow(dead_code)]
    cache: HashMap<String, Vec<CompletionItem>>,
    max_items: usize,
}

impl Default for CompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            max_items: 100,
        }
    }

    /// Format completion items for display
    pub fn format_completions(&self, items: &[CompletionItem]) -> Vec<FormattedCompletion> {
        items
            .iter()
            .take(self.max_items)
            .map(|item| {
                let kind = item
                    .kind
                    .as_ref()
                    .map(|k| format!("{:?}", k))
                    .unwrap_or_else(|| "Unknown".to_string());

                let detail = item.detail.clone().unwrap_or_default();
                let documentation = item
                    .documentation
                    .as_ref()
                    .and_then(|d| match d {
                        lsp_types::Documentation::String(s) => Some(s.clone()),
                        lsp_types::Documentation::MarkupContent(c) => Some(c.value.clone()),
                    })
                    .unwrap_or_default();

                FormattedCompletion {
                    label: item.label.clone(),
                    kind,
                    detail,
                    documentation,
                    insert_text: item
                        .insert_text
                        .clone()
                        .unwrap_or_else(|| item.label.clone()),
                    sort_text: item.sort_text.clone(),
                }
            })
            .collect()
    }

    /// Get top completions filtered by prefix
    pub fn get_matching(&self, prefix: &str, items: &[CompletionItem]) -> Vec<FormattedCompletion> {
        let prefix_lower = prefix.to_lowercase();

        let mut completions: Vec<_> = items
            .iter()
            .filter(|item| item.label.to_lowercase().starts_with(&prefix_lower))
            .take(self.max_items)
            .map(|item| FormattedCompletion {
                label: item.label.clone(),
                kind: format!(
                    "{:?}",
                    item.kind
                        .as_ref()
                        .unwrap_or(&lsp_types::CompletionItemKind::TEXT)
                ),
                detail: item.detail.clone().unwrap_or_default(),
                documentation: item
                    .documentation
                    .as_ref()
                    .and_then(|d| match d {
                        lsp_types::Documentation::String(s) => Some(s.clone()),
                        lsp_types::Documentation::MarkupContent(c) => Some(c.value.clone()),
                    })
                    .unwrap_or_default(),
                insert_text: item
                    .insert_text
                    .clone()
                    .unwrap_or_else(|| item.label.clone()),
                sort_text: item.sort_text.clone(),
            })
            .collect();

        // Sort by sort_text if available, otherwise label alphabetically
        completions.sort_by(|a, b| match (&a.sort_text, &b.sort_text) {
            (Some(sa), Some(sb)) => sa.cmp(sb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.label.cmp(&b.label),
        });
        completions
    }
}

#[derive(Debug, Clone)]
pub struct FormattedCompletion {
    pub label: String,
    pub kind: String,
    pub detail: String,
    pub documentation: String,
    pub insert_text: String,
    pub sort_text: Option<String>,
}

impl FormattedCompletion {
    /// Format for terminal display
    pub fn to_display_string(&self, max_width: usize) -> String {
        let mut output = format!("{} ", self.label);

        if !self.detail.is_empty() {
            let detail = if self.detail.len() > max_width / 2 {
                format!("{}...", &self.detail[..max_width / 2 - 3])
            } else {
                self.detail.clone()
            };
            output.push_str(&format!("- {}", detail));
        }

        output
    }
}
