//! `/plans` — surface the planner data plane.
//!
//! The `src/planner/` subsystem ships a markdown plan format and a
//! primitive to advance one phase at a time, but until now plans
//! were invisible to users — the files sat in `.d3vx/plans/*.md`
//! with no UI hook. This module adds the minimum-viable window:
//!
//! - `/plans` lists every plan file in the current project's
//!   `.d3vx/plans/` directory, one line each with id, status, title,
//!   and phase progress (`2/5 phases done`).
//! - `plans_count_active()` is used by the status strip to show an
//!   ambient "N plans" indicator when there's in-flight work.
//!
//! This is pure read-only visibility. Plan *execution* (wiring the
//! planner into the chat/vex loop) is a follow-up step — doing the
//! visibility cut first means the execution layer will land into a
//! UI that already shows its effects.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::app::App;
use crate::planner::{parse_plan, Plan, PlanStatus, SectionState};

/// Project-relative path where plan files live. Kept as a constant so
/// the status strip and the slash command agree on where to look.
const PLANS_DIR: &str = ".d3vx/plans";

/// `/plans` — show every plan file in `.d3vx/plans/` with status and
/// phase progress. Terse by design: one line per plan, no colour
/// overrides, so a user scanning `/plans` gets a density comparable
/// to `git branch` or `cargo test`.
pub fn handle_plans(app: &mut App, _args: &[&str]) -> Result<()> {
    let dir = plans_dir(app);
    let plans = match list_plans(&dir) {
        Ok(p) => p,
        Err(e) => {
            app.add_system_message(&format!(
                "No plans directory at {} ({e}). Run a task via `/plan <request>` to create one.",
                dir.display()
            ));
            return Ok(());
        }
    };

    if plans.is_empty() {
        app.add_system_message(&format!(
            "No plans yet. Plans live in {} and are created by the planner.",
            dir.display()
        ));
        return Ok(());
    }

    let mut out = format!("Plans ({}):\n", plans.len());
    for entry in &plans {
        out.push_str(&format_plan_row(entry));
        out.push('\n');
    }
    app.add_system_message(&out);
    Ok(())
}

/// Count of plans that aren't in a terminal state. Used by the
/// status strip; call it per-frame without worrying about cost —
/// a plans directory of any realistic size is a cheap read.
pub fn plans_count_active(project_cwd: Option<&str>) -> usize {
    let dir = resolve_plans_dir(project_cwd);
    match list_plans(&dir) {
        Ok(entries) => entries
            .into_iter()
            .filter(|p| !is_terminal(p.status))
            .count(),
        Err(_) => 0,
    }
}

/// Entry for a single row in the `/plans` listing. Kept separate
/// from `Plan` itself because the listing doesn't need the full
/// section bodies — only the summary counts.
#[derive(Debug, Clone)]
pub struct PlanListEntry {
    pub id: String,
    pub title: String,
    pub status: PlanStatus,
    pub completed: usize,
    pub total: usize,
}

/// Read and parse every `*.md` file under `dir`. Files that fail to
/// parse are skipped silently — a hand-edited plan that's currently
/// invalid shouldn't crash the listing (users can find the real
/// error by opening the file).
pub fn list_plans(dir: &Path) -> Result<Vec<PlanListEntry>> {
    let mut out: Vec<PlanListEntry> = Vec::new();
    let read_dir = fs::read_dir(dir)?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().map(|s| s != "md").unwrap_or(true) {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let plan = match parse_plan(&raw) {
            Ok(p) => p,
            Err(_) => continue,
        };
        out.push(entry_from_plan(&plan));
    }
    // Stable order: in-progress first, then drafts, then terminal,
    // alphabetical within each bucket. Makes the listing predictable
    // across invocations.
    out.sort_by(|a, b| {
        sort_key(a.status)
            .cmp(&sort_key(b.status))
            .then(a.id.cmp(&b.id))
    });
    Ok(out)
}

fn entry_from_plan(plan: &Plan) -> PlanListEntry {
    let completed = plan
        .sections
        .iter()
        .filter(|s| s.state == SectionState::Completed)
        .count();
    PlanListEntry {
        id: plan.id.clone(),
        title: plan.title.clone(),
        status: plan.status,
        completed,
        total: plan.sections.len(),
    }
}

fn format_plan_row(entry: &PlanListEntry) -> String {
    let status_str = status_label(entry.status);
    // Clamp title so long titles don't push progress off-screen on
    // narrow terminals. 48 chars is a comfortable read width.
    let title: String = entry.title.chars().take(48).collect();
    format!(
        "  [{}] {:<12} {:>2}/{:<2}  {}",
        entry.id, status_str, entry.completed, entry.total, title
    )
}

fn status_label(s: PlanStatus) -> &'static str {
    match s {
        PlanStatus::Draft => "draft",
        PlanStatus::InProgress => "in-progress",
        PlanStatus::Completed => "completed",
        PlanStatus::Failed => "failed",
        PlanStatus::Cancelled => "cancelled",
    }
}

fn sort_key(s: PlanStatus) -> u8 {
    match s {
        PlanStatus::InProgress => 0,
        PlanStatus::Draft => 1,
        PlanStatus::Failed => 2,
        PlanStatus::Completed => 3,
        PlanStatus::Cancelled => 4,
    }
}

fn is_terminal(s: PlanStatus) -> bool {
    matches!(
        s,
        PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Cancelled
    )
}

fn plans_dir(app: &App) -> PathBuf {
    resolve_plans_dir(app.cwd.as_deref())
}

fn resolve_plans_dir(project_cwd: Option<&str>) -> PathBuf {
    match project_cwd {
        Some(cwd) => PathBuf::from(cwd).join(PLANS_DIR),
        None => PathBuf::from(PLANS_DIR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use crate::pipeline::phases::Phase;
    use crate::planner::{serialize_plan, PhaseSelection, Plan};

    fn write_plan_file(dir: &Path, plan: &Plan) {
        let path = dir.join(format!("{}.md", plan.id));
        fs::write(path, serialize_plan(plan)).unwrap();
    }

    fn mk_plan(id: &str, title: &str, phases: Vec<Phase>) -> Plan {
        Plan::new(id, title, "req", PhaseSelection::from_phases(phases))
    }

    #[test]
    fn listing_an_empty_or_missing_dir_is_not_an_error_for_count() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("nonexistent");
        assert_eq!(plans_count_active(missing.to_str()), 0);
    }

    #[test]
    fn lists_all_plan_files_and_extracts_status_progress() {
        let temp = TempDir::new().unwrap();
        write_plan_file(
            temp.path(),
            &mk_plan(
                "alpha",
                "Alpha plan",
                vec![Phase::Plan, Phase::Implement],
            ),
        );
        write_plan_file(
            temp.path(),
            &mk_plan("beta", "Beta plan", vec![Phase::Implement]),
        );
        let entries = list_plans(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        // Sanity: both entries surface title + total phase count.
        assert!(entries.iter().any(|e| e.id == "alpha" && e.total == 2));
        assert!(entries.iter().any(|e| e.id == "beta" && e.total == 1));
    }

    #[test]
    fn in_progress_plans_come_before_terminal_ones() {
        let temp = TempDir::new().unwrap();
        let mut done = mk_plan("zzz-done", "done", vec![Phase::Implement]);
        done.record_outcome(0, SectionState::Completed, "body").unwrap();
        let mut active = mk_plan("aaa-active", "active", vec![Phase::Plan, Phase::Implement]);
        active.record_outcome(0, SectionState::Completed, "body").unwrap();

        write_plan_file(temp.path(), &done);
        write_plan_file(temp.path(), &active);

        let entries = list_plans(temp.path()).unwrap();
        // `active` is in-progress (1/2 done); `done` is completed (1/1).
        assert_eq!(entries[0].id, "aaa-active");
        assert_eq!(entries[1].id, "zzz-done");
    }

    #[test]
    fn unparseable_files_are_skipped_silently() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("broken.md"), "not a plan").unwrap();
        write_plan_file(
            temp.path(),
            &mk_plan("good", "Good plan", vec![Phase::Implement]),
        );
        let entries = list_plans(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "good");
    }

    #[test]
    fn plans_count_active_excludes_completed() {
        // `plans_count_active` takes the *project root* and appends
        // `.d3vx/plans` internally — mirror that real layout here so
        // the test exercises the full resolution path.
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join(".d3vx").join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        let mut done = mk_plan("done", "done", vec![Phase::Implement]);
        done.record_outcome(0, SectionState::Completed, "body").unwrap();
        let active = mk_plan("active", "active", vec![Phase::Implement]);

        write_plan_file(&plans_dir, &done);
        write_plan_file(&plans_dir, &active);

        assert_eq!(plans_count_active(temp.path().to_str()), 1);
    }
}
