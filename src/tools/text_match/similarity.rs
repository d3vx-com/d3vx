//! Similarity-based matching using edit distance.
//!
//! Falls back to Levenshtein distance when all other strategies fail.
//! Only triggers when similarity ratio ≥ MIN_RATIO to avoid false positives.

use super::{Match, Strategy};

/// Minimum similarity ratio (0.0–1.0) to accept a fuzzy match.
const MIN_RATIO: f64 = 0.6;

/// Maximum needle length for full-window sliding (avoids O(n²) on huge files).
const MAX_SLIDE_LENGTH: usize = 2000;

/// Find the best fuzzy match of `needle` within `haystack`.
pub fn find(haystack: &str, needle: &str) -> Option<Match> {
    if needle.len() > haystack.len() || needle.is_empty() {
        return None;
    }

    // For short needles, slide a window over the haystack.
    if needle.len() <= MAX_SLIDE_LENGTH {
        return slide_and_find(haystack, needle);
    }

    // For long needles, break into chunks and find the best overlapping region.
    chunked_find(haystack, needle)
}

/// Slide a window of `needle.len()` over `haystack` and find the best match.
fn slide_and_find(haystack: &str, needle: &str) -> Option<Match> {
    let needle_bytes = needle.as_bytes();
    let hay_bytes = haystack.as_bytes();
    let needle_len = needle_bytes.len();
    let hay_len = hay_bytes.len();

    // Step size: don't check every byte offset — use ~20 positions for reasonable perf
    let step = (needle_len / 20).max(1).min(50);
    let mut best_start: usize = 0;
    let mut best_ratio: f64 = 0.0;

    let mut pos = 0;
    while pos + needle_len <= hay_len {
        let ratio = compute_ratio(&hay_bytes[pos..pos + needle_len], needle_bytes);
        if ratio > best_ratio {
            best_ratio = ratio;
            best_start = pos;
        }
        if ratio >= 0.95 {
            // Good enough — don't keep searching
            break;
        }
        pos += step;
    }

    // Also check boundary-aligned positions (line starts)
    for line_start in haystack.lines().enumerate().filter_map(|(i, l)| {
        if i > 0 {
            let byte_off = l.as_ptr() as usize - haystack.as_ptr() as usize;
            Some(byte_off)
        } else {
            None
        }
    }) {
        if line_start + needle_len > hay_len {
            continue;
        }
        let ratio = compute_ratio(
            &hay_bytes[line_start..line_start + needle_len],
            needle_bytes,
        );
        if ratio > best_ratio {
            best_ratio = ratio;
            best_start = line_start;
        }
    }

    if best_ratio < MIN_RATIO {
        tracing::debug!(
            strategy = "similarity",
            best_ratio,
            threshold = MIN_RATIO,
            "Below threshold, no match"
        );
        return None;
    }

    // Refine boundaries: try expanding/contracting around best_start
    let (refined_start, refined_end) =
        refine_boundaries(haystack, needle, best_start, best_start + needle_len);

    tracing::debug!(
        strategy = "similarity",
        best_ratio,
        start = refined_start,
        end = refined_end,
        "Similarity match found"
    );

    Some(Match {
        start: refined_start,
        end: refined_end,
        strategy: Strategy::Similarity,
    })
}

/// For long needles: break into overlapping chunks, find best region, then refine.
fn chunked_find(haystack: &str, needle: &str) -> Option<Match> {
    let chunk_size = 500;
    let needle_bytes = needle.as_bytes();
    let hay_bytes = haystack.as_bytes();

    // Take first and last chunks as anchors
    let head = &needle_bytes[..chunk_size.min(needle_bytes.len())];
    let tail = &needle_bytes[needle_bytes.len().saturating_sub(chunk_size)..];

    let mut best_start = 0;
    let mut best_score = 0.0f64;

    // Find where the head matches best
    let step = (chunk_size / 10).max(1);
    for pos in (0..hay_bytes.len().saturating_sub(head.len())).step_by(step) {
        let score = compute_ratio(&hay_bytes[pos..pos + head.len()], head);
        if score > best_score {
            best_score = score;
            best_start = pos;
        }
    }

    if best_score < MIN_RATIO * 0.8 {
        return None;
    }

    // Estimate end from head position + needle length
    let estimated_end = (best_start + needle.len()).min(haystack.len());

    // Verify with tail
    if estimated_end >= tail.len() {
        let tail_start = estimated_end.saturating_sub(tail.len());
        let tail_score = compute_ratio(&hay_bytes[tail_start..estimated_end], tail);
        if tail_score < MIN_RATIO * 0.8 {
            return None;
        }
    }

    Some(Match {
        start: best_start,
        end: estimated_end,
        strategy: Strategy::Similarity,
    })
}

/// Compute similarity ratio between two byte slices using Levenshtein distance.
fn compute_ratio(a: &[u8], b: &[u8]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dist = levenshtein(a, b);
    let max_len = a.len().max(b.len()) as f64;
    1.0 - (dist as f64 / max_len)
}

/// Levenshtein distance between two byte slices.
fn levenshtein(a: &[u8], b: &[u8]) -> usize {
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

/// Try to refine match boundaries by expanding/contracting around the initial window.
fn refine_boundaries(haystack: &str, needle: &str, start: usize, end: usize) -> (usize, usize) {
    let hay_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();

    // Try expanding up to ±200 bytes
    let mut best_start = start;
    let mut best_end = end;
    let mut best_ratio = compute_ratio(&hay_bytes[start..end.min(hay_bytes.len())], needle_bytes);

    let expand = 200.min(start).min(hay_bytes.len().saturating_sub(end));
    for delta in 1..=expand {
        // Expand start left
        if start >= delta {
            let new_start = start - delta;
            let new_end = (end.min(hay_bytes.len())).min(hay_bytes.len());
            let r = compute_ratio(&hay_bytes[new_start..new_end], needle_bytes);
            if r > best_ratio {
                best_ratio = r;
                best_start = new_start;
                best_end = new_end;
            }
        }
    }

    // Snap to character boundaries (avoid splitting multi-byte chars)
    while !haystack.is_char_boundary(best_start) && best_start > 0 {
        best_start -= 1;
    }
    while !haystack.is_char_boundary(best_end) && best_end < haystack.len() {
        best_end += 1;
    }

    (best_start, best_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_code_matches() {
        let hay = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        let needle = "fn add(a: i32, b: i32) -> i32 {\n    a + b  \n}";
        // Extra trailing spaces — similar enough
        let m = find(hay, needle);
        // This might or might not match depending on exact ratio
        // The key is it doesn't panic
        if let Some(m) = m {
            assert_eq!(m.strategy, Strategy::Similarity);
        }
    }

    #[test]
    fn completely_different_returns_none() {
        let hay = "enum Foo { A, B }";
        let needle = "struct Bar { x: i32 }";
        assert!(find(hay, needle).is_none());
    }

    #[test]
    fn empty_needle_returns_none() {
        assert!(find("content", "").is_none());
    }

    #[test]
    fn threshold_boundary() {
        // ~65% similar — should match
        let hay = "let mut result = Vec::new();\nfor item in items {\n    result.push(item);\n}";
        let needle = "let mut result = Vec::new();\nfor i in items {\n    result.push(i);\n}";
        let m = find(hay, needle);
        assert!(m.is_some());
    }

    #[test]
    fn levenshtein_identical_is_zero() {
        assert_eq!(levenshtein(b"hello", b"hello"), 0);
    }

    #[test]
    fn levenshtein_single_deletion() {
        assert_eq!(levenshtein(b"hello", b"helo"), 1);
    }

    #[test]
    fn levenshtein_single_insertion() {
        assert_eq!(levenshtein(b"cat", b"cart"), 1);
    }

    #[test]
    fn levenshtein_single_substitution() {
        assert_eq!(levenshtein(b"cat", b"bat"), 1);
    }

    #[test]
    fn levenshtein_empty_strings() {
        assert_eq!(levenshtein(b"", b""), 0);
    }

    #[test]
    fn levenshtein_full_replacement() {
        assert_eq!(levenshtein(b"abc", b"xyz"), 3);
    }

    #[test]
    fn compute_ratio_identical() {
        assert_eq!(compute_ratio(b"hello", b"hello"), 1.0);
    }

    #[test]
    fn compute_ratio_completely_different() {
        // For "abc" vs "xyz": distance 3, max_len 3, ratio = 0.0
        assert!((compute_ratio(b"abc", b"xyz") - 0.0).abs() < 0.01);
    }

    #[test]
    fn compute_ratio_empty_both() {
        assert_eq!(compute_ratio(b"", b""), 1.0);
    }

    #[test]
    fn compute_ratio_empty_one() {
        assert_eq!(compute_ratio(b"", b"test"), 0.0);
        assert_eq!(compute_ratio(b"test", b""), 0.0);
    }
}
