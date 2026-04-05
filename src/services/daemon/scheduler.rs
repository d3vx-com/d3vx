//! Interval-based scheduler for daemon workers.

use std::collections::HashMap;

use tracing::{debug, info};

use super::workers::{DaemonWorker, WorkerContext, WorkerResult};

// ---------------------------------------------------------------------------
// Interval parsing
// ---------------------------------------------------------------------------

/// Parse a simplified cron expression and return the interval in minutes.
///
/// Supported forms:
/// - `*/N * * * *`  -> N minutes
/// - `0 * * * *`    -> 60 minutes (on the hour)
///
/// Falls back to 60 minutes for anything unrecognised.
pub fn parse_interval_minutes(schedule: &str) -> u64 {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() != 5 {
        return 60;
    }

    // `*/N * * * *` form.
    if let Some(rest) = fields[0].strip_prefix("*/") {
        if let Ok(n) = rest.parse::<u64>() {
            if n > 0 {
                return n;
            }
        }
    }

    // `0 * * * *` form (every hour).
    if fields[0] == "0" && fields[1] == "*" {
        return 60;
    }

    60
}

// ---------------------------------------------------------------------------
// WorkerInfo
// ---------------------------------------------------------------------------

/// Summary of a registered worker.
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub name: String,
    pub description: String,
    pub schedule: String,
    pub last_run: Option<String>,
}

// ---------------------------------------------------------------------------
// DaemonScheduler
// ---------------------------------------------------------------------------

/// Manages scheduling and execution of daemon workers.
///
/// Uses simplified interval-based scheduling: a worker is considered "due"
/// when at least its interval (parsed from the cron expression) has elapsed
/// since the last recorded run. Workers that have never run are always due.
pub struct DaemonScheduler {
    workers: Vec<Box<dyn DaemonWorker>>,
    last_run: HashMap<String, String>,
    enabled: bool,
}

impl DaemonScheduler {
    /// Create an empty, enabled scheduler.
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
            last_run: HashMap::new(),
            enabled: true,
        }
    }

    /// Create a scheduler pre-loaded with the given workers.
    pub fn with_workers(workers: Vec<Box<dyn DaemonWorker>>) -> Self {
        Self {
            workers,
            last_run: HashMap::new(),
            enabled: true,
        }
    }

    /// Register an additional worker.
    pub fn add_worker(&mut self, worker: Box<dyn DaemonWorker>) {
        info!(name = worker.name(), "Registering daemon worker");
        self.workers.push(worker);
    }

    /// Check all registered workers and execute any that are due.
    ///
    /// Returns results only for workers that were actually executed in this
    /// tick.
    pub fn tick(&mut self, ctx: &WorkerContext) -> Vec<WorkerResult> {
        if !self.enabled {
            debug!("Scheduler disabled; skipping tick");
            return Vec::new();
        }

        let now_ts = &ctx.timestamp;
        let mut results = Vec::new();

        for worker in &self.workers {
            if is_due(worker.as_ref(), &self.last_run, now_ts) {
                debug!(name = worker.name(), "Worker is due, executing");
                let result = worker.execute(ctx);
                self.last_run
                    .insert(worker.name().to_string(), now_ts.clone());
                results.push(result);
            }
        }

        results
    }

    /// Return summary information for every registered worker.
    pub fn list_workers(&self) -> Vec<WorkerInfo> {
        self.workers
            .iter()
            .map(|w| WorkerInfo {
                name: w.name().to_string(),
                description: w.description().to_string(),
                schedule: w.schedule().to_string(),
                last_run: self.last_run.get(w.name()).cloned(),
            })
            .collect()
    }

    /// Enable the scheduler.
    pub fn enable(&mut self) {
        self.enabled = true;
        info!("Daemon scheduler enabled");
    }

    /// Disable the scheduler (ticks become no-ops).
    pub fn disable(&mut self) {
        self.enabled = false;
        info!("Daemon scheduler disabled");
    }
}

// ---------------------------------------------------------------------------
// Scheduling helpers
// ---------------------------------------------------------------------------

/// Determine if a worker is due by comparing its interval against the elapsed
/// time since `last_run`. If the worker has never run, it is always due.
fn is_due(worker: &dyn DaemonWorker, last_run: &HashMap<String, String>, now_ts: &str) -> bool {
    let last = match last_run.get(worker.name()) {
        Some(ts) => ts,
        None => return true, // never run
    };

    let now_mins = parse_iso_to_minutes(now_ts);
    let last_mins = parse_iso_to_minutes(last);
    let interval = parse_interval_minutes(worker.schedule());

    now_mins.saturating_sub(last_mins) >= interval
}

/// Very small ISO-8601 timestamp parser that returns a value in minutes.
///
/// Expects format `YYYY-MM-DDTHH:MM:SSZ`. Only uses hours and minutes for
/// the calculation, which is sufficient for interval-based scheduling.
fn parse_iso_to_minutes(ts: &str) -> u64 {
    // Expected length: "2026-01-01T00:00:00Z" = 20 chars
    if ts.len() < 16 {
        return 0;
    }
    // Extract HH:MM portion starting at byte 11.
    let hour_str = ts.get(11..13).unwrap_or("00");
    let min_str = ts.get(14..16).unwrap_or("00");

    let hours: u64 = hour_str.parse().unwrap_or(0);
    let mins: u64 = min_str.parse().unwrap_or(0);

    // Add a large day offset so that dates within a few hours of midnight
    // boundary still compare correctly. This is intentionally simplified.
    let day_str = ts.get(8..10).unwrap_or("01");
    let days: u64 = day_str.parse().unwrap_or(1);

    (days * 24 * 60) + (hours * 60) + mins
}

// ---------------------------------------------------------------------------
// Default impl
// ---------------------------------------------------------------------------

impl Default for DaemonScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal test worker with a configurable schedule.
    struct FakeWorker {
        name: &'static str,
        schedule: &'static str,
    }

    impl FakeWorker {
        fn new(name: &'static str, schedule: &'static str) -> Self {
            Self { name, schedule }
        }
    }

    impl DaemonWorker for FakeWorker {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "fake worker for tests"
        }

        fn schedule(&self) -> &str {
            self.schedule
        }

        fn execute(&self, _ctx: &WorkerContext) -> WorkerResult {
            WorkerResult {
                status: super::super::workers::WorkerStatus::Success,
                message: "ok".to_string(),
                items_processed: 0,
            }
        }
    }

    fn ctx_at(ts: &str) -> WorkerContext {
        WorkerContext {
            project_root: std::path::PathBuf::from("/tmp/d3vx-test"),
            timestamp: ts.to_string(),
        }
    }

    #[test]
    fn test_scheduler_starts_empty() {
        let scheduler = DaemonScheduler::new();
        assert!(scheduler.workers.is_empty());
        assert!(scheduler.enabled);
        assert!(scheduler.list_workers().is_empty());
    }

    #[test]
    fn test_add_worker() {
        let mut scheduler = DaemonScheduler::new();
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/5 * * * *")));
        assert_eq!(scheduler.list_workers().len(), 1);
        assert_eq!(scheduler.list_workers()[0].name, "w1");
    }

    #[test]
    fn test_tick_executes_due_workers() {
        let mut scheduler = DaemonScheduler::new();
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/5 * * * *")));

        // Never run before -> should execute.
        let results = scheduler.tick(&ctx_at("2026-01-01T00:00:00Z"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].items_processed, 0);
    }

    #[test]
    fn test_tick_skips_not_due_workers() {
        let mut scheduler = DaemonScheduler::new();
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/10 * * * *")));

        // First tick: executes and records last_run.
        let _ = scheduler.tick(&ctx_at("2026-01-01T00:00:00Z"));

        // Second tick only 5 minutes later: should be skipped (interval=10).
        let results = scheduler.tick(&ctx_at("2026-01-01T00:05:00Z"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_enable_disable() {
        let mut scheduler = DaemonScheduler::new();
        assert!(scheduler.enabled);

        scheduler.disable();
        assert!(!scheduler.enabled);

        // Tick while disabled should return nothing.
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/5 * * * *")));
        let results = scheduler.tick(&ctx_at("2026-01-01T00:00:00Z"));
        assert!(results.is_empty());

        scheduler.enable();
        assert!(scheduler.enabled);

        let results = scheduler.tick(&ctx_at("2026-01-01T00:00:00Z"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_parse_interval_minutes_every_n() {
        assert_eq!(parse_interval_minutes("*/10 * * * *"), 10);
        assert_eq!(parse_interval_minutes("*/30 * * * *"), 30);
        assert_eq!(parse_interval_minutes("*/5 * * * *"), 5);
    }

    #[test]
    fn test_parse_interval_minutes_hourly() {
        assert_eq!(parse_interval_minutes("0 * * * *"), 60);
    }

    #[test]
    fn test_parse_interval_minutes_fallback() {
        assert_eq!(parse_interval_minutes("* * * * *"), 60);
        assert_eq!(parse_interval_minutes("garbage"), 60);
        assert_eq!(parse_interval_minutes(""), 60);
    }

    #[test]
    fn test_list_workers_includes_last_run() {
        let mut scheduler = DaemonScheduler::new();
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/5 * * * *")));

        // No runs yet.
        let info = &scheduler.list_workers()[0];
        assert!(info.last_run.is_none());

        // After tick, last_run should be populated.
        let _ = scheduler.tick(&ctx_at("2026-01-01T12:30:00Z"));
        let info = &scheduler.list_workers()[0];
        assert_eq!(info.last_run.as_deref(), Some("2026-01-01T12:30:00Z"));
    }

    #[test]
    fn test_default_impl() {
        let scheduler = DaemonScheduler::default();
        assert!(scheduler.enabled);
        assert!(scheduler.workers.is_empty());
    }

    #[test]
    fn test_worker_executes_again_after_interval() {
        let mut scheduler = DaemonScheduler::new();
        scheduler.add_worker(Box::new(FakeWorker::new("w1", "*/10 * * * *")));

        // First tick at 00:00.
        let r1 = scheduler.tick(&ctx_at("2026-01-01T00:00:00Z"));
        assert_eq!(r1.len(), 1);

        // 5 min later -> not due.
        let r2 = scheduler.tick(&ctx_at("2026-01-01T00:05:00Z"));
        assert!(r2.is_empty());

        // 10 min later -> due again.
        let r3 = scheduler.tick(&ctx_at("2026-01-01T00:10:00Z"));
        assert_eq!(r3.len(), 1);
    }
}
