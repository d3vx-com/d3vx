//! Mention picker and tab completion helpers

use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::app::slash_commands;
use crate::app::App;

impl App {
    pub(crate) fn handle_tab_completion(&mut self, reverse: bool) -> Result<()> {
        if self.ui.input_buffer.is_empty() {
            return Ok(());
        }

        // Simple slash command completion
        if self.ui.input_buffer.starts_with('/') {
            let current = &self.ui.input_buffer[1..];
            let matches: Vec<String> = slash_commands::SLASH_COMMANDS
                .iter()
                .filter(|c| c.name.starts_with(current))
                .map(|c| format!("/{}", c.name))
                .collect();

            if !matches.is_empty() {
                // Simplified cycle logic for first implementation
                let idx = if reverse {
                    matches.len().saturating_sub(1)
                } else {
                    0
                };
                self.ui.input_buffer = matches[idx].clone();
                self.ui.cursor_position = self.ui.input_buffer.len();
            }
        }
        Ok(())
    }

    pub(crate) fn active_token_range(&self) -> Option<(usize, usize, String)> {
        if self.ui.cursor_position > self.ui.input_buffer.len() {
            return None;
        }

        let bytes = self.ui.input_buffer.as_bytes();
        let mut start = self.ui.cursor_position;
        while start > 0 && !bytes[start - 1].is_ascii_whitespace() {
            start -= 1;
        }

        let mut end = self.ui.cursor_position;
        while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
            end += 1;
        }

        if start == end {
            return None;
        }

        Some((start, end, self.ui.input_buffer[start..end].to_string()))
    }

    pub(crate) fn clear_mention_picker(&mut self) {
        self.ui.mention_suggestions.clear();
        self.ui.mention_selected = 0;
    }

    pub(crate) fn refresh_mention_picker(&mut self) -> Result<()> {
        let Some((_, _, token)) = self.active_token_range() else {
            self.clear_mention_picker();
            return Ok(());
        };
        if !token.starts_with('@') {
            self.clear_mention_picker();
            return Ok(());
        }

        let suggestions = self.file_mention_candidates(&token[1..])?;
        self.ui.mention_suggestions = suggestions;
        if self.ui.mention_suggestions.is_empty() {
            self.ui.mention_selected = 0;
        } else if self.ui.mention_selected >= self.ui.mention_suggestions.len() {
            self.ui.mention_selected = 0;
        }
        Ok(())
    }

    pub(crate) fn file_mention_candidates(&self, fragment: &str) -> Result<Vec<String>> {
        if fragment.is_empty() {
            return Ok(Vec::new());
        }

        let cwd = self
            .cwd
            .clone()
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| ".".to_string());

        let base_dir = PathBuf::from(&cwd);
        let matcher = SkimMatcherV2::default();
        let mut matches = Vec::new();

        // Recursively walk the workspace
        // Skip common large/hidden directories for performance
        let walker = WalkDir::new(&base_dir).into_iter().filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".git" && name != "node_modules" && name != "target" && name != ".next"
        });

        for entry in walker.flatten() {
            let path = entry.path();

            // Get path relative to base_dir for cleaner suggestions and matching
            let relative_path = path
                .strip_prefix(&base_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());

            if relative_path.is_empty() || relative_path == "." {
                continue;
            }

            // Fuzzy match the relative path against the fragment
            if let Some(score) = matcher.fuzzy_match(&relative_path, fragment) {
                let completed = if path.is_dir() {
                    format!("{}/", relative_path)
                } else {
                    relative_path
                };
                matches.push((score, completed));
            }

            // Cap the total number of files we check to prevent UI hanging in massive projects
            if matches.len() > 500 {
                break;
            }
        }

        // Sort by score descending and truncate
        matches.sort_by(|a: &(i64, String), b: &(i64, String)| b.0.cmp(&a.0));

        let result: Vec<String> = matches.into_iter().take(15).map(|(_, path)| path).collect();

        Ok(result)
    }

    pub(crate) fn select_prev_mention(&mut self) -> bool {
        if self.ui.mention_suggestions.is_empty() {
            return false;
        }
        if self.ui.mention_selected == 0 {
            self.ui.mention_selected = self.ui.mention_suggestions.len().saturating_sub(1);
        } else {
            self.ui.mention_selected -= 1;
        }
        true
    }

    pub(crate) fn select_next_mention(&mut self, reverse: bool) -> bool {
        if self.ui.mention_suggestions.is_empty() {
            return false;
        }
        if reverse {
            return self.select_prev_mention();
        }
        self.ui.mention_selected =
            (self.ui.mention_selected + 1) % self.ui.mention_suggestions.len();
        true
    }

    pub(crate) fn accept_selected_mention(&mut self) -> Result<bool> {
        if self.ui.mention_suggestions.is_empty() {
            return Ok(false);
        }
        let Some((start, end, token)) = self.active_token_range() else {
            self.clear_mention_picker();
            return Ok(false);
        };
        if !token.starts_with('@') {
            self.clear_mention_picker();
            return Ok(false);
        }
        let selected = self
            .ui
            .mention_suggestions
            .get(self.ui.mention_selected)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("invalid mention selection"))?;
        self.ui
            .input_buffer
            .replace_range(start..end, &format!("@{}", selected));
        self.ui.cursor_position = start + selected.len() + 1;
        self.refresh_mention_picker()?;
        if self
            .ui
            .mention_suggestions
            .iter()
            .any(|candidate| candidate == &selected)
        {
            self.clear_mention_picker();
        }
        Ok(true)
    }
}
