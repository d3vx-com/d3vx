//! Code Map Scoring and Query Logic
//!
//! Tokenization, scoring functions, call-bonus, and query ranking.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Result;
use tracing::{debug, info, trace};
use walkdir::WalkDir;

use crate::services::analysis::SymbolExtractor;

use super::types::{is_keyword, is_supported_source, CodeMap, FileEntry, ScoredFile, SKIP_DIRS};

// -- Tokenisation -----------------------------------------------------------

/// Split a string into lowercase terms on whitespace, camelCase, snake_case, hyphen boundaries.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() || ch == '_' || ch == '-' {
            if !cur.is_empty() {
                terms.push(cur.to_lowercase());
                cur.clear();
            }
        } else if ch.is_uppercase() && !cur.is_empty() {
            terms.push(cur.to_lowercase());
            cur.clear();
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        terms.push(cur.to_lowercase());
    }
    terms.retain(|t| t.len() >= 2);
    terms
}

// -- Scoring ----------------------------------------------------------------

/// Depth penalty: `0.8^depth`.
pub fn depth_penalty(depth: usize) -> f64 {
    0.8_f64.powi(depth as i32)
}

/// Size factor: `sqrt(lines / (identifiers + 1))`.
pub fn size_factor(lines: usize, id_count: usize) -> f64 {
    (lines as f64 / (id_count + 1) as f64).sqrt()
}

/// Compute the base score for a file against query terms.
pub fn compute_base_score(entry: &FileEntry, query_terms: &[String]) -> (f64, Vec<String>) {
    let id_set: HashSet<String> = entry
        .identifiers
        .iter()
        .chain(entry.defined_symbols.iter())
        .map(|s| s.to_lowercase())
        .collect();
    let matched: Vec<String> = query_terms
        .iter()
        .filter(|t| {
            id_set
                .iter()
                .any(|id| id.contains(t.as_str()) || t.contains(id.as_str()))
        })
        .cloned()
        .collect();
    let freq = if query_terms.is_empty() {
        0.0
    } else {
        matched.len() as f64 / query_terms.len() as f64
    };
    let score = freq * depth_penalty(entry.depth)
        / size_factor(entry.line_count, entry.identifiers.len()).max(1.0);
    (score, matched)
}

/// Apply one PageRank-like iteration: files whose symbols are called by high-scoring files get a bonus.
pub fn apply_call_bonus(files: &mut HashMap<PathBuf, FileEntry>) {
    let sym_map: HashMap<String, PathBuf> = files
        .values()
        .flat_map(|e| {
            e.defined_symbols
                .iter()
                .map(|s| (s.to_lowercase(), e.path.clone()))
        })
        .collect();
    let mut bonuses: HashMap<PathBuf, f64> = HashMap::new();
    for entry in files.values() {
        if entry.score <= 0.0 {
            continue;
        }
        for call in &entry.calls {
            if let Some(p) = sym_map.get(&call.to_lowercase()) {
                if p != &entry.path {
                    *bonuses.entry(p.clone()).or_insert(0.0) += entry.score * 0.1;
                }
            }
        }
    }
    for (path, bonus) in bonuses {
        if let Some(e) = files.get_mut(&path) {
            e.score += bonus;
        }
    }
}

// -- Building the code map --------------------------------------------------

/// Walk the project directory, extract symbols from each supported source file.
pub fn build_code_map(project_root: &std::path::Path) -> Result<CodeMap> {
    info!("Building code map for {:?}", project_root);
    let mut extractor = SymbolExtractor::new();
    let mut files = HashMap::new();
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !n.starts_with('.') && !SKIP_DIRS.contains(&n.as_ref())
        })
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() || !is_supported_source(path) {
            continue;
        }
        trace!("Processing file: {:?}", path);
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                debug!("Skip {:?}: {}", path, e);
                continue;
            }
        };
        let depth = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .components()
            .count()
            .saturating_sub(1);
        let symbols = extractor.extract(path, &content);
        files.insert(
            path.to_path_buf(),
            FileEntry {
                path: path.to_path_buf(),
                depth,
                line_count: content.lines().count(),
                identifiers: extract_identifiers(&content),
                calls: extract_calls(&content),
                defined_symbols: symbols.iter().map(|s| s.name.clone()).collect(),
                score: 0.0,
            },
        );
    }
    info!("Code map built: {} files", files.len());
    Ok(CodeMap { files })
}

fn extract_identifiers(content: &str) -> Vec<String> {
    let mut ids = HashSet::new();
    for part in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let lo = part.to_lowercase();
        if lo.len() >= 3 && !is_keyword(&lo) {
            ids.insert(lo);
        }
    }
    ids.into_iter().collect()
}

fn extract_calls(content: &str) -> Vec<String> {
    let mut calls = HashSet::new();
    let bytes = content.as_bytes();
    let (len, mut i) = (bytes.len(), 0);
    while i < len {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let mut j = i;
            while j < len && bytes[j] == b' ' {
                j += 1;
            }
            if j < len && bytes[j] == b'(' {
                let name = String::from_utf8_lossy(&bytes[start..i]).to_lowercase();
                if name.len() >= 2 && !is_keyword(&name) {
                    calls.insert(name);
                }
            }
        } else {
            i += 1;
        }
    }
    calls.into_iter().collect()
}

// -- Query ranking ----------------------------------------------------------

/// Rank files by relevance to a query. Returns at most `max_results` sorted by descending score.
pub fn rank_files_for_query(
    query: &str,
    code_map: &CodeMap,
    max_results: usize,
) -> Vec<ScoredFile> {
    let terms = tokenize(query);
    debug!("Query terms: {:?}", terms);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<ScoredFile> = code_map
        .files
        .values()
        .filter_map(|e| {
            let (s, m) = compute_base_score(e, &terms);
            (s > 0.0).then(|| ScoredFile {
                path: e.path.clone(),
                score: s,
                matched_terms: m,
            })
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(max_results);
    scored
}
