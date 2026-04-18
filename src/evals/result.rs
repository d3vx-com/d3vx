//! Results and aggregate reporting for eval runs.
//!
//! `EvalResult` captures one task's outcome; `EvalReport` aggregates a
//! batch. Reports render to Markdown (human), TSV (spreadsheet), or
//! JSON (programmatic) so operators and CI pipelines can consume the
//! same data without parsing table layouts.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use super::grader::GradeOutcome;

/// Outcome of running one eval task end-to-end.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub task_id: String,
    pub task_name: String,
    /// Overall pass/fail — `true` iff every grader outcome passed and
    /// no harness-level error occurred.
    pub passed: bool,
    /// Per-grader verdicts, in task-declared order.
    pub grader_outcomes: Vec<GradeOutcome>,
    /// Wall-clock duration from provisioning to the last grader.
    pub duration_ms: u64,
    /// Reported agent cost in USD, if known.
    pub cost_usd: Option<f64>,
    /// Iterations the agent ran, if reported by the runtime.
    pub iterations: Option<u32>,
    /// Total tool calls, if reported by the runtime.
    pub tool_calls: Option<u32>,
    /// Harness-level error when the task couldn't run to grading
    /// (provisioning failed, agent crashed, timeout, etc.). Present
    /// implies `passed = false`.
    #[serde(default)]
    pub harness_error: Option<String>,
}

impl EvalResult {
    /// Construct a passing result with empty cost/iteration fields.
    pub fn success(
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        grader_outcomes: Vec<GradeOutcome>,
        duration_ms: u64,
    ) -> Self {
        let passed = grader_outcomes.iter().all(|g| g.passed);
        Self {
            task_id: task_id.into(),
            task_name: task_name.into(),
            passed,
            grader_outcomes,
            duration_ms,
            cost_usd: None,
            iterations: None,
            tool_calls: None,
            harness_error: None,
        }
    }

    /// Construct a failure result from a harness-level error.
    pub fn harness_failure(
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            task_name: task_name.into(),
            passed: false,
            grader_outcomes: Vec::new(),
            duration_ms,
            cost_usd: None,
            iterations: None,
            tool_calls: None,
            harness_error: Some(error.into()),
        }
    }

    /// Builder-style cost setter.
    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = Some(cost_usd);
        self
    }

    /// Builder-style iteration count setter.
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = Some(iterations);
        self
    }

    /// Builder-style tool-call count setter.
    pub fn with_tool_calls(mut self, tool_calls: u32) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }
}

/// Aggregate of a batch of eval results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub results: Vec<EvalResult>,
}

/// Output format for [`EvalReport::render`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Markdown,
    Tsv,
    Json,
}

impl EvalReport {
    pub fn new(results: Vec<EvalResult>) -> Self {
        Self { results }
    }

    /// Number of tasks that passed.
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    /// Fraction of tasks that passed, in `[0.0, 1.0]`. Returns 0.0 on
    /// empty reports (rather than panic/NaN) so comparison code can
    /// be simple.
    pub fn pass_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        self.passed_count() as f64 / self.results.len() as f64
    }

    /// Sum of reported costs across results. Results with unknown cost
    /// contribute nothing (rather than guessing).
    pub fn total_cost_usd(&self) -> f64 {
        self.results.iter().filter_map(|r| r.cost_usd).sum()
    }

    /// One-line summary for stdout / notifications.
    pub fn summary(&self) -> String {
        let pct = (self.pass_rate() * 100.0).round();
        format!(
            "{}/{} passed ({pct:.0}%)  |  total cost: ${:.2}",
            self.passed_count(),
            self.results.len(),
            self.total_cost_usd(),
        )
    }

    /// Render the report in the requested format.
    pub fn render(&self, format: ReportFormat) -> String {
        match format {
            ReportFormat::Markdown => self.render_markdown(),
            ReportFormat::Tsv => self.render_tsv(),
            ReportFormat::Json => self.render_json(),
        }
    }

    fn render_markdown(&self) -> String {
        let mut out = String::with_capacity(256 + self.results.len() * 96);
        writeln!(&mut out, "# Eval report").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(&mut out, "{}", self.summary()).unwrap();
        writeln!(&mut out).unwrap();
        writeln!(
            &mut out,
            "| # | Task | Result | Cost | Iter | Tools | Duration |"
        )
        .unwrap();
        writeln!(&mut out, "|---|------|--------|------|------|-------|----------|").unwrap();
        for (idx, r) in self.results.iter().enumerate() {
            writeln!(
                &mut out,
                "| {} | {} | {} | {} | {} | {} | {:.2}s |",
                idx + 1,
                r.task_name,
                if r.passed { "✅ pass" } else { "❌ fail" },
                r.cost_usd
                    .map(|c| format!("${c:.3}"))
                    .unwrap_or_else(|| "—".into()),
                r.iterations.map(|i| i.to_string()).unwrap_or_else(|| "—".into()),
                r.tool_calls
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "—".into()),
                r.duration_ms as f64 / 1000.0,
            )
            .unwrap();
        }
        out
    }

    fn render_tsv(&self) -> String {
        let mut out = String::with_capacity(64 + self.results.len() * 64);
        writeln!(
            &mut out,
            "task_id\ttask_name\tpassed\tduration_ms\tcost_usd\titerations\ttool_calls\terror"
        )
        .unwrap();
        for r in &self.results {
            writeln!(
                &mut out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                r.task_id,
                r.task_name,
                r.passed,
                r.duration_ms,
                r.cost_usd.map(|c| format!("{c:.4}")).unwrap_or_default(),
                r.iterations.map(|i| i.to_string()).unwrap_or_default(),
                r.tool_calls.map(|i| i.to_string()).unwrap_or_default(),
                r.harness_error.clone().unwrap_or_default(),
            )
            .unwrap();
        }
        out
    }

    fn render_json(&self) -> String {
        // Unwrap is safe: our types derive Serialize over primitive
        // types only, and serde_json cannot fail on those. Using
        // `expect` rather than `unwrap` to document the invariant.
        serde_json::to_string_pretty(self).expect("EvalReport serialises to JSON")
    }
}
