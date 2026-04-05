//! Candidate extraction for error context.
//!
//! When all matching strategies fail, extract the region of `haystack` most
//! similar to `needle` so the caller can show it in an error message.

/// Context lines to include around a candidate match.
const CONTEXT_LINES: usize = 3;

/// Maximum total snippet length for error messages.
const MAX_SNIPPET_LEN: usize = 600;

/// Find the region of `haystack` most similar to `needle`.
///
/// Returns a string snippet with surrounding context, Useful for
/// error messages so the LLM can self-correct.
pub fn find_nearest(haystack: &str, needle: &str) -> Option<String> {
    if needle.is_empty() || haystack.is_empty() {
        return None;
    }

    let needle_len = needle.len();
    let hay_len = haystack.len();
    if needle_len > hay_len {
        return None;
    }

    // Slide a window of ~needle.len() over haystack and track best position
    let step = (needle_len / 10).max(1).min(50);
    let mut best_pos: usize = 0;
    let mut best_ratio: f64 = 0.0;

    let hay_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();

    let mut pos = 0;
    while pos + needle_len <= hay_len {
        let window = &hay_bytes[pos..pos + needle_len];
        let ratio = quick_ratio(window, needle_bytes);
        if ratio > best_ratio {
            best_ratio = ratio;
            best_pos = pos;
        }
        pos += step;
    }

    if best_ratio < 0.2 {
        return None;
    }

    // Extract lines around best_pos for context
    let lines: Vec<&str> = haystack.lines().collect();
    let line_offsets = build_line_byte_offsets(haystack);

    // Find the line containing best_pos
    let center_line = line_offsets
        .iter()
        .enumerate()
        .filter_map(|(i, &offset)| {
            if offset <= best_pos && i + 1 < line_offsets.len() {
                Some(i)
            } else {
                None
            }
        })
        .last()
        .unwrap_or(0);

    let start_line = center_line.saturating_sub(CONTEXT_LINES).max(1);
    let end_line = (start_line + CONTEXT_LINES * 2 + 1).min(lines.len());

    // Build snippet with line numbers
    let mut snippet = String::new();
    for i in start_line..end_line {
        if i < lines.len() {
            snippet.push_str(&format!("{:4}│ {}\n", i + 1, lines[i]));
        }
    }

    // Truncate if too long
    if snippet.len() > MAX_SNIPPET_LEN {
        snippet.truncate(MAX_SNIPPET_LEN);
    }

    Some(snippet)
}

/// Compute a quick byte-level similarity ratio between two slices.
fn quick_ratio(a: &[u8], b: &[u8]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Sample at most 20 positions for speed
    let sample_count = 20.min(a.len()).min(b.len()).max(1);
    let step_a = (a.len() / sample_count).max(1);
    let step_b = (b.len() / sample_count).max(1);

    let mut matches = 0usize;
    let total = sample_count;

    for i in 0..sample_count {
        let pos_a = (i * step_a).min(a.len() - 1);
        let pos_b = (i * step_b).min(b.len() - 1);
        if a.get(pos_a) == b.get(pos_b) {
            matches += 1;
        }
    }

    matches as f64 / total as f64
}

/// Build byte offsets for the start of each line.
fn build_line_byte_offsets(text: &str) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(text.lines().count() + 1);
    let mut byte_pos = 0;

    for line in text.lines() {
        offsets.push(byte_pos);
        byte_pos += line.len() + 1; // +1 for '\n'
    }

    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nearby_region() {
        let hay = "fn foo() {\n    bar()\n}\nfn baz() {\n    qux()\n}";
        let needle = "fn bar() {\n    bar()\n}";
        let result = find_nearest(hay, needle).unwrap();
        assert!(result.contains("bar"));
    }

    #[test]
    fn returns_none_for_empty() {
        assert!(find_nearest("", "needle").is_none());
        assert!(find_nearest("haystack", "").is_none());
    }

    #[test]
    fn returns_none_for_too_different() {
        assert!(find_nearest("aaaa bbbb cccc", "xyz 123 456").is_none());
    }

    #[test]
    fn snippet_is_bounded() {
        let hay: String = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let needle = "line 50\nline 51";
        let result = find_nearest(&hay, needle).unwrap();
        assert!(result.len() <= MAX_SNIPPET_LEN);
    }
}
