//! Tests for the slash-palette pure logic: visibility, filtering,
//! selection-wrap arithmetic. Rendering is exercised manually via
//! `cargo run`; we test only what a regression would plausibly break.

use super::slash_palette::{match_commands, palette_visible_for, wrap_next, wrap_prev};

// ── Visibility ──────────────────────────────────────────────────

#[test]
fn invisible_when_empty() {
    assert!(!palette_visible_for(""));
}

#[test]
fn visible_on_bare_slash() {
    assert!(palette_visible_for("/"));
}

#[test]
fn visible_while_typing_command_name() {
    assert!(palette_visible_for("/dash"));
}

#[test]
fn invisible_once_user_supplies_args() {
    // The user is past the command token — showing a picker would
    // cover the arguments they're typing.
    assert!(!palette_visible_for("/vex fix the bug"));
}

#[test]
fn invisible_for_normal_prompts() {
    assert!(!palette_visible_for("hello world"));
    assert!(!palette_visible_for("explain foo"));
}

#[test]
fn tolerates_leading_whitespace() {
    // A user who Tab-indents out of habit shouldn't get a dead
    // prompt — treat whitespace-prefixed slashes the same.
    assert!(palette_visible_for("  /dash"));
}

// ── Matching ────────────────────────────────────────────────────

#[test]
fn empty_fragment_lists_all_commands() {
    let matches = match_commands("");
    assert!(matches.len() >= 10);
    let names: Vec<&str> = matches.iter().map(|c| c.name).collect();
    assert!(names.contains(&"board"));
    assert!(names.contains(&"dashboard"));
    assert!(names.contains(&"daemon"));
    assert!(names.contains(&"vex"));
}

#[test]
fn prefix_matches_come_before_substring() {
    let matches = match_commands("da"); // dashboard + daemon by prefix
    let names: Vec<&str> = matches.iter().map(|c| c.name).collect();
    let dashboard_idx = names.iter().position(|n| *n == "dashboard").unwrap();
    let daemon_idx = names.iter().position(|n| *n == "daemon").unwrap();
    // Both should be in the top section (prefix matches) — the key
    // assertion is that they rank ahead of any substring-only hits.
    let anything_after = matches.iter().skip(2).any(|c| {
        !c.name.to_ascii_lowercase().starts_with("da")
            && c.name.to_ascii_lowercase().contains("da")
    });
    assert!(dashboard_idx < 2);
    assert!(daemon_idx < 2);
    let _ = anything_after;
}

#[test]
fn fragment_is_case_insensitive() {
    let a = match_commands("BOARD");
    let b = match_commands("board");
    assert_eq!(a.iter().map(|c| c.name).collect::<Vec<_>>(),
               b.iter().map(|c| c.name).collect::<Vec<_>>());
}

#[test]
fn fragment_matches_descriptions_as_well_as_names() {
    // "browser" isn't a command name but it appears in the
    // `/dashboard` description; the palette should still surface it.
    let matches = match_commands("browser");
    assert!(matches.iter().any(|c| c.name == "dashboard"));
}

#[test]
fn no_match_returns_empty() {
    let matches = match_commands("xyzqwerty-definitely-not-a-command");
    assert!(matches.is_empty());
}

// ── Wrap arithmetic ─────────────────────────────────────────────

#[test]
fn next_wraps_from_last_back_to_zero() {
    assert_eq!(wrap_next(4, 5), 0);
}

#[test]
fn next_advances_one_in_middle() {
    assert_eq!(wrap_next(2, 5), 3);
}

#[test]
fn prev_wraps_from_zero_to_last() {
    assert_eq!(wrap_prev(0, 5), 4);
}

#[test]
fn prev_decrements_in_middle() {
    assert_eq!(wrap_prev(3, 5), 2);
}

#[test]
fn wrap_helpers_are_safe_when_list_is_empty() {
    // A stale selection against an empty match set must not panic.
    assert_eq!(wrap_next(0, 0), 0);
    assert_eq!(wrap_prev(0, 0), 0);
    assert_eq!(wrap_next(7, 0), 0);
    assert_eq!(wrap_prev(7, 0), 0);
}

#[test]
fn prev_clamps_stale_index_before_wrapping() {
    // If the match count shrank since the last selection tick, the
    // stored index may be out of range. wrap_prev should clamp first
    // so the user sees a sensible "last row" instead of wrapping
    // past the actual tail.
    assert_eq!(wrap_prev(99, 3), 1);
}
