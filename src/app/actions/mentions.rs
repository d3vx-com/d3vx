//! File mention resolution and image path extraction

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::FocusMode;

const MAX_MENTION_FILES: usize = 8;
const MAX_MENTION_FILE_BYTES: usize = 12_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MentionResolution {
    pub expanded_prompt: String,
    pub resolved_paths: Vec<String>,
    pub unresolved: Vec<String>,
}

pub(crate) fn sanitize_mention_token(token: &str) -> Option<String> {
    if !token.starts_with('@') || token == "@" {
        return None;
    }

    let mention = token[1..]
        .trim_end_matches(|c: char| matches!(c, ',' | ';' | ':' | ')' | ']' | '}'))
        .trim();

    if mention.is_empty() || mention.starts_with('@') {
        None
    } else {
        Some(mention.to_string())
    }
}

fn infer_code_fence_language(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "json" => "json",
        "md" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" => "bash",
        "html" => "html",
        "css" => "css",
        _ => "",
    }
}

pub(crate) fn extract_image_paths(content: &str, cwd: Option<&str>) -> (String, Vec<PathBuf>) {
    let mut images = Vec::new();
    let mut remaining_words = Vec::new();

    let base_dir = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut current_token = String::new();
    let mut in_quotes: Option<char> = None;
    let mut escape = false;

    let mut tokens = Vec::new();

    for c in content.chars() {
        if escape {
            current_token.push(c);
            escape = false;
            continue;
        }

        if c == '\\' {
            escape = true;
            continue;
        }

        if let Some(q) = in_quotes {
            if c == q {
                in_quotes = None;
            } else {
                current_token.push(c);
            }
            continue;
        }

        if c == '"' || c == '\'' {
            in_quotes = Some(c);
            continue;
        }

        if c.is_whitespace() {
            if !current_token.is_empty() {
                tokens.push(std::mem::take(&mut current_token));
            }
        } else {
            current_token.push(c);
        }
    }
    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    for token in tokens {
        let is_image_ext = {
            let lower = token.to_lowercase();
            lower.ends_with(".png")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
                || lower.ends_with(".webp")
                || lower.ends_with(".gif")
        };

        if is_image_ext {
            let path = PathBuf::from(&token);
            let canonical = if path.is_absolute() {
                path
            } else {
                base_dir.join(&path)
            };

            if canonical.exists() && canonical.is_file() {
                images.push(canonical);
                continue;
            }
        }

        if token.contains(' ') {
            remaining_words.push(format!("\"{}\"", token));
        } else {
            remaining_words.push(token);
        }
    }

    (remaining_words.join(" "), images)
}

pub(crate) fn resolve_file_mentions(content: &str, cwd: Option<&str>) -> MentionResolution {
    let base_dir = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let normalized_base_dir = base_dir.canonicalize().unwrap_or_else(|_| base_dir.clone());

    let mut resolved_blocks = Vec::new();
    let mut resolved_paths = Vec::new();
    let mut unresolved = Vec::new();
    let mut seen = HashSet::new();

    for token in content.split_whitespace() {
        let Some(mention) = sanitize_mention_token(token) else {
            continue;
        };
        if !seen.insert(mention.clone()) {
            continue;
        }
        if resolved_blocks.len() >= MAX_MENTION_FILES {
            unresolved.push(format!("{} (mention limit exceeded)", mention));
            continue;
        }

        let candidate = if Path::new(&mention).is_absolute() {
            PathBuf::from(&mention)
        } else {
            base_dir.join(&mention)
        };

        let absolute = candidate.canonicalize().unwrap_or(candidate);
        if !absolute.is_file() {
            unresolved.push(mention);
            continue;
        }

        let bytes = match fs::read(&absolute) {
            Ok(bytes) => bytes,
            Err(_) => {
                unresolved.push(mention);
                continue;
            }
        };

        let truncated = bytes.len() > MAX_MENTION_FILE_BYTES;
        let file_content =
            String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_MENTION_FILE_BYTES)]).to_string();
        let display_path = absolute
            .strip_prefix(&normalized_base_dir)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|_| absolute.to_string_lossy().to_string());
        let language = infer_code_fence_language(&absolute);

        resolved_paths.push(display_path.clone());
        let mut block = format!("\nFile: {}\n", display_path);
        if truncated {
            block.push_str("(truncated to the first 12000 bytes)\n");
        }
        block.push_str(&format!("```{}\n{}\n```\n", language, file_content));
        resolved_blocks.push(block);
    }

    if resolved_blocks.is_empty() {
        return MentionResolution {
            expanded_prompt: content.to_string(),
            resolved_paths,
            unresolved,
        };
    }

    let mut expanded_prompt = String::from(content);
    expanded_prompt.push_str("\n\nReferenced file context:\n");
    for block in resolved_blocks {
        expanded_prompt.push_str(&block);
    }

    MentionResolution {
        expanded_prompt,
        resolved_paths,
        unresolved,
    }
}

pub(crate) fn apply_focus_mode_to_prompt(content: &str, focus_mode: FocusMode) -> String {
    let Some(instruction) = focus_mode.system_instruction() else {
        return content.to_string();
    };

    format!(
        "{}\n\nFocus-mode task context:\n- Active preset: {}\n- Guidance: {}\n",
        content,
        focus_mode.label(),
        instruction
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolves_file_mentions_into_prompt_context() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("main.rs");
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let resolution = resolve_file_mentions("inspect @main.rs", temp_dir.path().to_str());
        assert_eq!(resolution.resolved_paths, vec!["main.rs".to_string()]);
        assert!(resolution.unresolved.is_empty());
        assert!(resolution
            .expanded_prompt
            .contains("Referenced file context:"));
        assert!(resolution.expanded_prompt.contains("fn main() {}"));
    }

    #[test]
    fn reports_unresolved_file_mentions() {
        let temp_dir = TempDir::new().unwrap();
        let resolution = resolve_file_mentions("inspect @missing.rs", temp_dir.path().to_str());
        assert!(resolution.resolved_paths.is_empty());
        assert_eq!(resolution.unresolved, vec!["missing.rs".to_string()]);
    }

    #[test]
    fn applies_focus_mode_context_to_prompt() {
        let prompt = apply_focus_mode_to_prompt("fix flaky tests", FocusMode::Test);
        assert!(prompt.contains("Focus-mode task context"));
        assert!(prompt.contains("Active preset: Test"));
        assert!(prompt.contains("fix flaky tests"));
    }
}
