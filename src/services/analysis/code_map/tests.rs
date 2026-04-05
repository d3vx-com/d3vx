//! Code Map Scoring Tests

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::services::analysis::code_map::scoring::*;
use crate::services::analysis::code_map::types::{is_supported_source, CodeMap, FileEntry};

fn make_entry(
    path: &str,
    depth: usize,
    lines: usize,
    ids: &[&str],
    calls: &[&str],
    defined: &[&str],
) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        depth,
        line_count: lines,
        score: 0.0,
        identifiers: ids.iter().map(|s| s.to_string()).collect(),
        calls: calls.iter().map(|s| s.to_string()).collect(),
        defined_symbols: defined.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn test_depth_penalty() {
    assert!((depth_penalty(0) - 1.0).abs() < f64::EPSILON);
    assert!((depth_penalty(1) - 0.8).abs() < f64::EPSILON);
    assert!((depth_penalty(2) - 0.64).abs() < f64::EPSILON);
    assert!(depth_penalty(5) < depth_penalty(3));
}

#[test]
fn test_size_factor() {
    assert!(size_factor(100, 50) < size_factor(100, 10));
    assert!((size_factor(100, 0) - 10.0).abs() < f64::EPSILON);
}

#[test]
fn test_deeper_files_score_lower() {
    let s = make_entry("a.rs", 0, 100, &["parse"], &[], &["parse"]);
    let d = make_entry("src/a/b/c/d.rs", 3, 100, &["parse"], &[], &["parse"]);
    let t = vec!["parse".to_string()];
    assert!(compute_base_score(&s, &t).0 > compute_base_score(&d, &t).0);
}

#[test]
fn test_matching_identifiers_rank_higher() {
    let m = make_entry(
        "m.rs",
        0,
        50,
        &["parse_tokens", "tokenize"],
        &[],
        &["parse_tokens"],
    );
    let o = make_entry(
        "o.rs",
        0,
        50,
        &["format_output", "serialize"],
        &[],
        &["format_output"],
    );
    let t = vec!["parse".to_string(), "token".to_string()];
    let (ms, matched) = compute_base_score(&m, &t);
    assert!(ms > compute_base_score(&o, &t).0);
    assert!(!matched.is_empty());
}

#[test]
fn test_top_n_truncation() {
    let files: HashMap<PathBuf, FileEntry> = [
        make_entry("a.rs", 0, 50, &["alpha"], &[], &["alpha"]),
        make_entry("b.rs", 0, 50, &["beta"], &[], &["beta"]),
        make_entry("c.rs", 0, 50, &["gamma"], &[], &["gamma"]),
    ]
    .into_iter()
    .map(|e| (e.path.clone(), e))
    .collect();
    let r = rank_files_for_query("alpha beta gamma", &CodeMap { files }, 2);
    assert_eq!(r.len(), 2);
    assert!(r[0].score >= r[1].score);
}

#[test]
fn test_empty_query_returns_nothing() {
    let files = HashMap::from([(
        PathBuf::from("a.rs"),
        make_entry("a.rs", 0, 50, &["alpha"], &[], &["alpha"]),
    )]);
    assert!(rank_files_for_query("", &CodeMap { files }, 10).is_empty());
}

#[test]
fn test_tokenize() {
    assert_eq!(
        tokenize("parseTokens snake_case hyphen-term"),
        vec!["parse", "tokens", "snake", "case", "hyphen", "term"]
    );
}

#[test]
fn test_tokenize_filters_short() {
    assert_eq!(tokenize("a b cd"), vec!["cd"]);
}

#[test]
fn test_apply_call_bonus() {
    let mut files = HashMap::from([
        (
            PathBuf::from("caller.rs"),
            FileEntry {
                path: PathBuf::from("caller.rs"),
                depth: 0,
                line_count: 50,
                identifiers: vec!["helper".into()],
                calls: vec!["process_data".into()],
                defined_symbols: vec!["main_fn".into()],
                score: 10.0,
            },
        ),
        (
            PathBuf::from("callee.rs"),
            FileEntry {
                path: PathBuf::from("callee.rs"),
                depth: 0,
                line_count: 30,
                identifiers: vec!["process_data".into()],
                calls: vec![],
                defined_symbols: vec!["process_data".into()],
                score: 1.0,
            },
        ),
    ]);
    apply_call_bonus(&mut files);
    assert!(files.get(&PathBuf::from("callee.rs")).unwrap().score > 1.0);
    assert!((files.get(&PathBuf::from("caller.rs")).unwrap().score - 10.0).abs() < f64::EPSILON);
}

#[test]
fn test_is_supported_source() {
    assert!(is_supported_source(Path::new("foo.rs")));
    assert!(is_supported_source(Path::new("bar.ts")));
    assert!(is_supported_source(Path::new("baz.py")));
    assert!(!is_supported_source(Path::new("readme.md")));
    assert!(!is_supported_source(Path::new("config.toml")));
}
