//! Tests for `agent_strip` rendering helpers.

use super::agent_strip::truncate_strip_task;

#[test]
fn truncate_short_task_is_unchanged() {
    assert_eq!(truncate_strip_task("Fix bug", 18), "Fix bug");
}

#[test]
fn truncate_long_task_breaks_on_word_boundary() {
    assert_eq!(
        truncate_strip_task("Refactor the authentication module", 18),
        "Refactor the.."
    );
}

#[test]
fn truncate_single_long_word_hard_cuts() {
    assert_eq!(
        truncate_strip_task("superlongwordthatdoesnotfit", 10),
        "superlon.."
    );
}
