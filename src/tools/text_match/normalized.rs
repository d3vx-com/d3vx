//! Normalized matching: collapses whitespace runs and trims lines.
//!
//! Maps back to original byte offsets so the caller can do precise replacement.

use super::{Match, Strategy};

/// Normalize a string: trim each line, collapse intra-line whitespace runs to single space.
fn normalize(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for line in input.lines() {
        if !result.is_empty() {
            result.push('\n');
        }
        // Trim line and collapse whitespace runs within the line
        let trimmed = line.trim();
        let mut prev_space = false;
        for ch in trimmed.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    result.push(' ');
                    prev_space = true;
                }
            } else {
                result.push(ch);
                prev_space = false;
            }
        }
    }
    result
}

/// Find `needle` in `haystack` using whitespace-normalized matching.
///
/// Strategy: normalize both strings, find match position, then walk original
/// line-by-line to find the corresponding byte range.
pub fn find(haystack: &str, needle: &str) -> Option<Match> {
    let norm_hay = normalize(haystack);
    let norm_needle = normalize(needle);

    // Find in normalized space
    let norm_match_start = norm_hay.find(&norm_needle)?;

    // Count lines before the match start in normalized haystack
    let norm_prefix = &norm_hay[..norm_match_start];
    let norm_start_line = norm_prefix.lines().count().saturating_sub(1);
    let needle_line_count = norm_needle.lines().count();

    // Map to original lines
    let orig_lines: Vec<(usize, usize)> = {
        let mut lines = Vec::new();
        let mut pos = 0;
        for line in haystack.lines() {
            lines.push((pos, pos + line.len()));
            pos += line.len() + 1; // +1 for '\n'
        }
        lines
    };

    if norm_start_line >= orig_lines.len() {
        return None;
    }

    let start_byte = orig_lines[norm_start_line].0;
    let end_line_idx = (norm_start_line + needle_line_count).min(orig_lines.len());
    let end_byte = if end_line_idx < orig_lines.len() {
        orig_lines[end_line_idx].0
    } else {
        haystack.len()
    };

    // Verify range is valid
    if start_byte > haystack.len() || end_byte > haystack.len() || start_byte > end_byte {
        return None;
    }

    tracing::debug!(
        strategy = "normalized",
        norm_start_line,
        start_byte,
        end_byte,
        "Whitespace-normalized match found"
    );

    Some(Match {
        start: start_byte,
        end: end_byte,
        strategy: Strategy::Normalized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_spaces_between_tokens() {
        // Spaces between tokens, not around punctuation — these normalize the same way
        let hay = "let  x  =  5;";
        let needle = "let x = 5;";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::Normalized);
        assert!(!hay[m.start..m.end].is_empty());
    }

    #[test]
    fn trailing_whitespace_per_line() {
        let hay = "line1  \nline2  \n";
        let needle = "line1\nline2\n";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::Normalized);
    }

    #[test]
    fn no_match_still_returns_none() {
        assert!(find("foo bar", "completely different").is_none());
    }

    #[test]
    fn tabs_vs_spaces() {
        let hay = "fn\tmain()\t{\n\tbody\n}";
        let needle = "fn main() {\n  body\n}";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::Normalized);
    }

    #[test]
    fn exact_match_not_normalized() {
        // When exact match works, normalized should still return a valid range
        let hay = "fn main() {}";
        let needle = "fn main() {}";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::Normalized);
        assert_eq!(&hay[m.start..m.end], "fn main() {}");
    }
}
