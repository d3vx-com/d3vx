//! Approximate string matching engine for edit operations.
//!
//! Provides a cascade of matching strategies, from fast/exact to slower/fuzzy.
//! Used by EditTool and MultiEditTool to tolerate minor differences between
//! the LLM's `old_string` and the actual file content.

mod candidate;
mod line_anchor;
mod normalized;
mod similarity;

/// Byte range within a haystack string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// Which strategy produced this match.
    pub strategy: Strategy,
}

impl Match {
    /// Extract the matched text from the haystack.
    pub fn text<'a>(&self, haystack: &'a str) -> &'a str {
        &haystack[self.start..self.end]
    }
}

/// Which matching strategy succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Exact byte-for-byte match.
    Exact,
    /// Match after normalizing whitespace (collapse runs, trim lines).
    Normalized,
    /// Match by comparing whole lines (tolerates indentation differences).
    LineAnchor,
    /// Best-effort match using edit distance (threshold ≥ 0.6).
    Similarity,
}

impl Strategy {
    /// Human-readable label for error messages and metadata.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Normalized => "normalized",
            Self::LineAnchor => "line-anchor",
            Self::Similarity => "similarity",
        }
    }
}

/// Try every matching strategy in cascade order, return first hit.
///
/// Order: Exact → Normalized → LineAnchor → Similarity
/// Each step is slower but more tolerant than the previous.
pub fn find_match(haystack: &str, needle: &str) -> Option<Match> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    // Strategy 1: Exact (free)
    if let Some(start) = haystack.find(needle) {
        return Some(Match {
            start,
            end: start + needle.len(),
            strategy: Strategy::Exact,
        });
    }

    // Strategy 2: Normalized (whitespace-insensitive, fast)
    if let Some(m) = normalized::find(haystack, needle) {
        return Some(m);
    }

    // Strategy 3: Line-anchor (tolerates indentation differences)
    if let Some(m) = line_anchor::find(haystack, needle) {
        return Some(m);
    }

    // Strategy 4: Similarity (Levenshtein fallback)
    if let Some(m) = similarity::find(haystack, needle) {
        return Some(m);
    }

    None
}

/// When all strategies fail, extract the region most similar to `needle`.
///
/// Returns a snippet of `haystack` with surrounding context, useful for
/// error messages so the LLM can self-correct.
pub fn find_nearest(haystack: &str, needle: &str) -> Option<String> {
    candidate::find_nearest(haystack, needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_returns_exact_strategy() {
        let hay = "Hello, World!";
        let m = find_match(hay, "World").unwrap();
        assert_eq!(m.strategy, Strategy::Exact);
        assert_eq!(&hay[m.start..m.end], "World");
    }

    #[test]
    fn empty_needle_returns_none() {
        assert!(find_match("content", "").is_none());
    }

    #[test]
    fn needle_longer_than_haystack_returns_none() {
        assert!(find_match("hi", "hello world").is_none());
    }

    #[test]
    fn no_match_returns_none() {
        assert!(find_match("foo bar baz", "completely different").is_none());
    }

    #[test]
    fn whitespace_difference_uses_normalized() {
        let hay = "fn  main  ( )  {\n    println!  ( \"hello\" ) ;\n}";
        let needle = "fn main ( ) {\n    println! ( \"hello\" ) ;\n}";
        let m = find_match(hay, needle).unwrap();
        assert_ne!(m.strategy, Strategy::Exact);
    }
}
