//! Line-anchor matching: compares content line-by-line.
//!
//! Tolerates:
//! - Different indentation (leading whitespace stripped per line)
//! - Trailing whitespace differences
//! - Arbitrary insertions/removals of blank lines
//!
//! Instead of sliding fixed-size windows, uses a two-pointer alignment starting
//! from each haystack position. This naturally handles any number of blank lines.

use super::{Match, Strategy};

/// Minimum ratio of matching needle lines to accept (0.0–1.0).
const MIN_LINE_RATIO: f64 = 0.7;

/// Find `needle` in `haystack` using line-by-line comparison.
pub fn find(haystack: &str, needle: &str) -> Option<Match> {
    let hay_lines: Vec<&str> = haystack.lines().collect();
    let needle_lines: Vec<&str> = needle.lines().collect();

    if needle_lines.is_empty() || needle_lines.len() > hay_lines.len() {
        return None;
    }

    let line_offsets = build_line_offsets(haystack);

    let mut best: Option<(usize, usize, f64)> = None; // (start_line, lines_consumed, score)

    for start in 0..hay_lines.len() {
        let (consumed, score) = align_and_score(&hay_lines[start..], &needle_lines);
        if score >= MIN_LINE_RATIO {
            match best {
                Some((_, _, best_score)) if score <= best_score => {}
                _ => best = Some((start, consumed, score)),
            }
        }
    }

    let (start_line, lines_consumed, score) = best?;
    let end_line = start_line + lines_consumed;

    let start_byte = line_offsets
        .get(start_line)
        .copied()
        .unwrap_or(haystack.len());
    let end_byte = if end_line < line_offsets.len() {
        line_offsets[end_line]
    } else {
        haystack.len()
    };

    // Don't include trailing newline if needle doesn't end with one
    let matched = &haystack[start_byte..end_byte];
    let matched = matched.strip_suffix('\n').unwrap_or(matched);
    let matched = matched.strip_suffix('\r').unwrap_or(matched);
    let end_byte = start_byte + matched.len();

    tracing::debug!(
        strategy = "line-anchor",
        start_line,
        end_line,
        score,
        "Line-anchor match found"
    );

    Some(Match {
        start: start_byte,
        end: end_byte,
        strategy: Strategy::LineAnchor,
    })
}

/// Build a vector of byte offsets for the start of each line.
fn build_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Two-pointer alignment: match needle lines against haystack lines.
///
/// Blank haystack lines are skipped (treated as insertions). Returns
/// (haystack_lines_consumed, score) where score = matched_needle_lines / needle_lines_total.
fn align_and_score(hay_lines: &[&str], needle_lines: &[&str]) -> (usize, f64) {
    let mut matches = 0usize;
    let mut ni = 0;
    let mut hi = 0;

    while ni < needle_lines.len() && hi < hay_lines.len() {
        let hay_trimmed = hay_lines[hi].trim();
        let needle_trimmed = needle_lines[ni].trim();

        if hay_trimmed.is_empty() && !needle_trimmed.is_empty() {
            // Blank haystack line — skip (insertion), don't advance needle
            hi += 1;
            continue;
        }

        if hay_trimmed == needle_trimmed {
            matches += 1;
        } else if line_similarity(hay_trimmed, needle_trimmed) > 0.6 {
            matches += 1;
        }
        ni += 1;
        hi += 1;
    }

    (hi, matches as f64 / needle_lines.len() as f64)
}

/// Quick character-level similarity between two trimmed strings.
fn line_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein_distance(a.as_bytes(), b.as_bytes());
    1.0 - (dist as f64 / max_len as f64)
}

/// Simple Levenshtein distance (byte-level) for short strings.
fn levenshtein_distance(a: &[u8], b: &[u8]) -> usize {
    let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
    let mut prev: Vec<usize> = (0..=short.len()).collect();
    let mut curr: Vec<usize> = vec![0; short.len() + 1];

    for (j, &lb) in long.iter().enumerate() {
        curr[0] = j + 1;
        for (i, &sa) in short.iter().enumerate() {
            let cost = if sa == lb { 0 } else { 1 };
            curr[i + 1] = (prev[i + 1] + 1).min(curr[i] + 1).min(prev[i] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[short.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_indentation() {
        let hay = "    fn main() {\n        println!(\"hello\");\n    }";
        let needle = "fn main() {\n    println!(\"hello\");\n}";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::LineAnchor);
    }

    #[test]
    fn extra_blank_lines() {
        let hay = "fn main() {\n\n    body()\n\n}";
        let needle = "fn main() {\n    body()\n}";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::LineAnchor);
    }

    #[test]
    fn no_match_returns_none() {
        assert!(find("foo\nbar", "completely\ndifferent").is_none());
    }

    #[test]
    fn single_line_match() {
        let hay = "    let x = 5;";
        let needle = "let x = 5;";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::LineAnchor);
    }

    #[test]
    fn many_blank_lines() {
        let hay = "fn main() {\n\n\n\n    body()\n\n\n\n}";
        let needle = "fn main() {\n    body()\n}";
        let m = find(hay, needle).unwrap();
        assert_eq!(m.strategy, Strategy::LineAnchor);
    }
}
