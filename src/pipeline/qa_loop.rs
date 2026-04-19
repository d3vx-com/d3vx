//! QA Loop Module
//!
//! Provides iterative quality assurance: review → fix → re-review → merge.
//!
//! ## Design Principles
//!
//! 1. **Bounded retries** - Max retry limit prevents infinite loops
//! 2. **Explicit state** - Every iteration has clear state and history
//! 3. **User visibility** - Current phase, findings, and blockers are surfaced
//! 4. **Safe escalation** - Unresolvable issues escalate cleanly
//!
//! ## Flow
//!
//! ```text
//! Implement → Validate → Review → [blocked] → Fix → ReReview → [approved] → Done
//!                                        → [blocked after max] → Escalate
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use pipeline::qa_loop::{QALoop, QAConfig};
//! use pipeline::merge_gate::MergeGate;
//!
//! let config = QAConfig::default();
//! let mut qa = QALoop::new("task-1".to_string(), config);
//! let gate = MergeGate::with_defaults();
//!
//! // Review phase
//! qa.start_review();
//! let readiness = gate.evaluate(&review_summary, &validation_summary, &docs);
//! if !readiness.ready {
//!     qa.record_merge_readiness(&readiness);
//!     // Agent fixes issues...
//!     qa.start_fix();
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::merge_gate::{MergeBlockingReason, MergeReadiness};
use super::review_gate::BlockingReason;
use super::review_summary::ReviewSummary;
use super::validation_summary::ValidationSummary;

const DEFAULT_MAX_RETRIES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QAState {
    Pending,
    InReview,
    AwaitingFix,
    InFix,
    ReReview,
    Approved,
    Escalated,
}

impl Default for QAState {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for QAState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InReview => write!(f, "in_review"),
            Self::AwaitingFix => write!(f, "awaiting_fix"),
            Self::InFix => write!(f, "in_fix"),
            Self::ReReview => write!(f, "re_review"),
            Self::Approved => write!(f, "approved"),
            Self::Escalated => write!(f, "escalated"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QAPhase {
    Review,
    Fix,
    ReReview,
}

impl std::fmt::Display for QAPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Review => write!(f, "review"),
            Self::Fix => write!(f, "fix"),
            Self::ReReview => write!(f, "re_review"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QALoopRecord {
    pub iteration: u32,
    pub phase: QAPhase,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub blocked: bool,
    pub blocking_reasons: Vec<String>,
    pub findings_summary: String,
    pub validation_confidence: Option<String>,
    pub duration_ms: Option<u64>,
    pub merge_readiness_summary: Option<String>,
    pub resolved_finding_ids: Vec<String>,
    pub still_blocking_ids: Vec<String>,
}

impl QALoopRecord {
    pub fn new(iteration: u32, phase: QAPhase) -> Self {
        Self {
            iteration,
            phase,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            blocked: false,
            blocking_reasons: Vec::new(),
            findings_summary: String::new(),
            validation_confidence: None,
            duration_ms: None,
            merge_readiness_summary: None,
            resolved_finding_ids: Vec::new(),
            still_blocking_ids: Vec::new(),
        }
    }

    pub fn complete(
        &mut self,
        blocked: bool,
        reasons: &[MergeBlockingReason],
        readiness: &MergeReadiness,
    ) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.blocked = blocked;
        self.blocking_reasons = reasons.iter().map(|r| r.code.clone()).collect();
        self.merge_readiness_summary = Some(readiness.summary.clone());
    }

    pub fn record_fix_resolution(&mut self, resolution: &FixResolutionStatus) {
        self.resolved_finding_ids = resolution.resolved_ids.clone();
        self.still_blocking_ids = resolution.still_blocking_ids.clone();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAConfig {
    pub max_retries: u32,
    pub require_validation: bool,
    pub auto_fix_on_nonblocking: bool,
}

impl Default for QAConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            require_validation: true,
            auto_fix_on_nonblocking: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QALoop {
    pub task_id: String,
    pub state: QAState,
    pub current_iteration: u32,
    pub max_retries: u32,
    pub history: Vec<QALoopRecord>,
    #[serde(skip)]
    pub current_phase_start: Option<Instant>,
    pub pending_findings: Vec<PendingFinding>,
    pub escalation_reason: Option<String>,
    pub last_validation: Option<ValidationSummary>,
    pub last_review: Option<ReviewSummary>,
    pub last_docs: Option<super::docs_completeness::DocsCompleteness>,
    pub last_merge_readiness: Option<super::merge_gate::MergeReadiness>,
}

impl QALoop {
    pub fn new(task_id: String, config: QAConfig) -> Self {
        Self {
            task_id,
            state: QAState::Pending,
            current_iteration: 0,
            max_retries: config.max_retries,
            history: Vec::new(),
            current_phase_start: None,
            pending_findings: Vec::new(),
            escalation_reason: None,
            last_validation: None,
            last_review: None,
            last_docs: None,
            last_merge_readiness: None,
        }
    }

    pub fn from_metadata(_task_id: String, metadata: &serde_json::Value) -> Option<Self> {
        metadata
            .get("qa_loop")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn to_metadata(&self) -> serde_json::Value {
        let mut metadata = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        if let Some(readiness) = &self.last_merge_readiness {
            if let Ok(readiness_json) = serde_json::to_value(readiness) {
                metadata["merge_readiness"] = readiness_json;
            }
        }
        metadata
    }

    pub fn iteration(&self) -> u32 {
        self.current_iteration
    }

    pub fn can_continue(&self) -> bool {
        self.state != QAState::Approved && self.state != QAState::Escalated
    }

    pub fn should_escalate(&self) -> bool {
        self.current_iteration >= self.max_retries && self.state != QAState::Approved
    }

    pub fn start_review(&mut self) {
        self.state = QAState::InReview;
        self.current_iteration += 1;
        self.current_phase_start = Some(Instant::now());
    }

    pub fn start_fix(&mut self) {
        self.state = QAState::InFix;
        self.current_phase_start = Some(Instant::now());
    }

    pub fn start_rereview(&mut self) {
        self.state = QAState::ReReview;
        self.current_phase_start = Some(Instant::now());
    }

    pub fn record_review_result(
        &mut self,
        review: &ReviewSummary,
        gate_result: &super::review_gate::GateResult,
    ) {
        self.last_review = Some(review.clone());
        let readiness =
            super::merge_gate::MergeGate::with_defaults().evaluate_from_gate_result(gate_result);
        self.last_merge_readiness = Some(readiness.clone());
        self.record_history(QAPhase::Review, gate_result.blocked, &readiness);
    }

    pub fn record_fix_result(
        &mut self,
        validation: Option<&ValidationSummary>,
        docs: Option<&super::docs_completeness::DocsCompleteness>,
    ) {
        if let Some(v) = validation {
            self.last_validation = Some(v.clone());
        }
        if let Some(d) = docs {
            self.last_docs = Some(d.clone());
        }
        let readiness = super::merge_gate::MergeGate::with_defaults().evaluate(
            self.last_review.as_ref(),
            self.last_validation.as_ref(),
            self.last_docs.as_ref(),
        );
        self.last_merge_readiness = Some(readiness.clone());
        self.record_history(QAPhase::Fix, false, &readiness);
    }

    pub fn record_docs_result(&mut self, docs: &super::docs_completeness::DocsCompleteness) {
        self.last_docs = Some(docs.clone());
        let readiness = super::merge_gate::MergeGate::with_defaults().evaluate(
            self.last_review.as_ref(),
            self.last_validation.as_ref(),
            self.last_docs.as_ref(),
        );
        self.last_merge_readiness = Some(readiness);
    }

    pub fn get_merge_readiness(&self) -> MergeReadiness {
        self.last_merge_readiness
            .clone()
            .unwrap_or_else(|| self.evaluate_merge_readiness())
    }

    pub fn evaluate_merge_readiness(&self) -> MergeReadiness {
        let gate = super::merge_gate::MergeGate::with_defaults();
        gate.evaluate(
            self.last_review.as_ref(),
            self.last_validation.as_ref(),
            self.last_docs.as_ref(),
        )
    }

    pub fn check_and_transition_from_readiness(
        &mut self,
        readiness: &MergeReadiness,
    ) -> QATransition {
        if readiness.ready {
            self.state = QAState::Approved;
            return QATransition::Approved;
        }

        if self.should_escalate() {
            self.escalation_reason = Some(format!(
                "Max retries ({}) exceeded with {} blocking issues",
                self.max_retries,
                readiness.reasons.len()
            ));
            self.state = QAState::Escalated;
            return QATransition::Escalated;
        }

        self.state = QAState::AwaitingFix;
        let reasons: Vec<_> = readiness
            .reasons
            .iter()
            .map(|r| super::review_gate::BlockingReason {
                code: r.code.clone(),
                message: r.message.clone(),
                category: match r.source {
                    super::merge_gate::MergeSource::Review => {
                        super::review_summary::FindingCategory::Coverage
                    }
                    super::merge_gate::MergeSource::Validation => {
                        super::review_summary::FindingCategory::Coverage
                    }
                    super::merge_gate::MergeSource::Docs => {
                        super::review_summary::FindingCategory::Documentation
                    }
                },
                finding_ids: Vec::new(),
            })
            .collect();
        QATransition::NeedsFix {
            reasons,
            iteration: self.current_iteration,
        }
    }

    fn record_history(&mut self, phase: QAPhase, blocked: bool, readiness: &MergeReadiness) {
        // Some code paths don't call start_* explicitly, so treat an
        // absent timer as "started now" (zero elapsed). Single-line
        // form prevents future refactors from accidentally removing
        // the fallback and reintroducing the panic.
        let start = self
            .current_phase_start
            .take()
            .unwrap_or_else(Instant::now);
        let elapsed = start.elapsed().as_millis() as u64;

        let mut record = QALoopRecord::new(self.current_iteration, phase);
        let reasons: Vec<_> = readiness
            .reasons
            .iter()
            .map(|r| MergeBlockingReason {
                source: r.source,
                code: r.code.clone(),
                message: r.message.clone(),
                can_auto_fix: r.can_auto_fix,
            })
            .collect();
        record.complete(blocked, &reasons, readiness);
        record.duration_ms = Some(elapsed);
        record.findings_summary = self.build_findings_summary();
        if let Some(v) = &self.last_validation {
            record.validation_confidence = Some(format!("{:?}", v.confidence));
        }
        self.history.push(record);
    }

    fn build_findings_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(review) = &self.last_review {
            let counts = review.count_by_severity();
            if counts.iter().any(|&c| c > 0) {
                parts.push(format!(
                    "findings: {} critical, {} high, {} medium, {} low",
                    counts[0], counts[1], counts[2], counts[3]
                ));
            }
        }

        if let Some(val) = &self.last_validation {
            parts.push(format!("validation: {:?}", val.confidence));
        }

        if self.pending_findings.is_empty() {
            parts.push("no pending fixes".to_string());
        } else {
            parts.push(format!("{} pending fixes", self.pending_findings.len()));
        }

        parts.join("; ")
    }

    pub fn add_pending_finding(&mut self, finding: PendingFinding) {
        if !self.pending_findings.iter().any(|f| f.id == finding.id) {
            self.pending_findings.push(finding);
        }
    }

    pub fn clear_pending_finding(&mut self, finding_id: &str) {
        self.pending_findings.retain(|f| f.id != finding_id);
    }

    pub fn add_pending_findings(&mut self, findings: &[PendingFinding]) {
        for finding in findings {
            self.add_pending_finding(finding.clone());
        }
    }

    pub fn get_pending_finding_ids(&self) -> Vec<String> {
        self.pending_findings.iter().map(|f| f.id.clone()).collect()
    }

    pub fn update_from_validation(&mut self, validation: &ValidationSummary) {
        self.last_validation = Some(validation.clone());

        if let Some(v) = &self.last_validation {
            if v.confidence.blocks_merge() && self.state == QAState::InFix {
                self.state = QAState::AwaitingFix;
            }
        }
    }

    /// Populate pending_findings from blocking reasons in the gate result.
    fn populate_pending_findings(&mut self, gate_result: &super::review_gate::GateResult) {
        for reason in &gate_result.reasons {
            for finding_id in &reason.finding_ids {
                if !self.pending_findings.iter().any(|f| f.id == *finding_id) {
                    self.pending_findings.push(PendingFinding {
                        id: finding_id.clone(),
                        title: reason.message.clone(),
                        category: format!("{:?}", reason.category),
                        severity: format!("{:?}", reason.category),
                        suggestion: None,
                        created_at_iteration: self.current_iteration,
                    });
                }
            }
        }
    }

    pub fn prepare_rereview(&mut self) -> Vec<PendingFinding> {
        let pending = self.pending_findings.clone();
        self.start_rereview();
        pending
    }

    pub fn verify_fix_resolution(&self, review: &ReviewSummary) -> FixResolutionStatus {
        let mut resolved = Vec::new();
        let mut still_blocking = Vec::new();

        for pending in &self.pending_findings {
            if let Some(finding) = review.findings.iter().find(|f| f.id == pending.id) {
                if finding.resolved {
                    resolved.push(pending.id.clone());
                } else if finding.severity.blocks_merge() {
                    still_blocking.push(pending.id.clone());
                }
            } else {
                resolved.push(pending.id.clone());
            }
        }

        let all_resolved = still_blocking.is_empty();
        FixResolutionStatus {
            resolved_ids: resolved,
            still_blocking_ids: still_blocking,
            all_resolved,
        }
    }

    pub fn check_and_transition(
        &mut self,
        gate_result: &super::review_gate::GateResult,
    ) -> QATransition {
        if gate_result.ready {
            if self.state == QAState::ReReview {
                let ids_to_clear: Vec<String> =
                    self.pending_findings.iter().map(|f| f.id.clone()).collect();
                for id in ids_to_clear {
                    self.clear_pending_finding(&id);
                }
            }
            self.state = QAState::Approved;
            return QATransition::Approved;
        }

        if self.should_escalate() {
            self.escalation_reason = Some(format!(
                "Max retries ({}) exceeded with {} blocking issues",
                self.max_retries,
                gate_result.reasons.len()
            ));
            self.state = QAState::Escalated;
            return QATransition::Escalated;
        }

        // Populate pending findings from review findings that contribute
        // to the blocking reasons.
        self.populate_pending_findings(gate_result);

        self.state = QAState::AwaitingFix;
        QATransition::NeedsFix {
            reasons: gate_result.reasons.clone(),
            iteration: self.current_iteration,
        }
    }

    pub fn handle_rereview_result(
        &mut self,
        review: &super::review_summary::ReviewSummary,
        gate_result: &super::review_gate::GateResult,
    ) -> QATransition {
        let resolution = self.verify_fix_resolution(review);

        // Clear resolved pending findings even when gate is still blocked
        for id in &resolution.resolved_ids {
            self.clear_pending_finding(id);
        }

        if gate_result.ready {
            let ids_to_clear: Vec<String> =
                self.pending_findings.iter().map(|f| f.id.clone()).collect();
            for id in ids_to_clear {
                self.clear_pending_finding(&id);
            }

            if let Some(record) = self.history.last_mut() {
                record.record_fix_resolution(&resolution);
            }

            self.state = QAState::Approved;
            return QATransition::Approved;
        }

        if resolution.all_resolved && !gate_result.reasons.is_empty() {
            let new_blockers: Vec<_> = gate_result
                .reasons
                .iter()
                .filter(|r| {
                    !resolution
                        .still_blocking_ids
                        .iter()
                        .any(|id| r.finding_ids.contains(id))
                })
                .cloned()
                .collect();

            if new_blockers.is_empty() && self.should_escalate() {
                self.escalation_reason = Some(format!(
                    "Max retries ({}) exceeded - fixes verified but new blockers appeared",
                    self.max_retries
                ));
                self.state = QAState::Escalated;
                return QATransition::Escalated;
            }

            if new_blockers.is_empty() {
                self.state = QAState::Approved;
                return QATransition::Approved;
            }
        }

        if let Some(record) = self.history.last_mut() {
            record.record_fix_resolution(&resolution);
        }

        self.state = QAState::AwaitingFix;
        QATransition::NeedsFix {
            reasons: gate_result.reasons.clone(),
            iteration: self.current_iteration,
        }
    }

    pub fn current_status(&self) -> QAStatus {
        QAStatus {
            state: self.state,
            iteration: self.current_iteration,
            max_retries: self.max_retries,
            pending_fixes: self.pending_findings.len(),
            is_merge_ready: self.state == QAState::Approved,
            needs_escalation: self.should_escalate(),
            escalation_reason: self.escalation_reason.clone(),
            last_findings_summary: self.history.last().map(|r| r.findings_summary.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingFinding {
    pub id: String,
    pub title: String,
    pub category: String,
    pub severity: String,
    pub suggestion: Option<String>,
    pub created_at_iteration: u32,
}

impl PendingFinding {
    pub fn from_blocking_reason(
        reason: &super::review_gate::BlockingReason,
        iteration: u32,
    ) -> Self {
        Self {
            id: reason
                .finding_ids
                .first()
                .cloned()
                .unwrap_or_else(|| format!("finding-{}", uuid::Uuid::new_v4().as_simple())),
            title: reason.message.clone(),
            category: format!("{:?}", reason.category),
            severity: format!("{:?}", reason.category),
            suggestion: None,
            created_at_iteration: iteration,
        }
    }

    pub fn from_review_finding(
        finding: &super::review_summary::ReviewFinding,
        iteration: u32,
    ) -> Self {
        Self {
            id: finding.id.clone(),
            title: finding.title.clone(),
            category: format!("{:?}", finding.category),
            severity: format!("{:?}", finding.severity),
            suggestion: finding.suggestion.clone(),
            created_at_iteration: iteration,
        }
    }
}

#[derive(Debug, Clone)]
pub enum QATransition {
    Approved,
    NeedsFix {
        reasons: Vec<BlockingReason>,
        iteration: u32,
    },
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QAStatus {
    pub state: QAState,
    pub iteration: u32,
    pub max_retries: u32,
    pub pending_fixes: usize,
    pub is_merge_ready: bool,
    pub needs_escalation: bool,
    pub escalation_reason: Option<String>,
    pub last_findings_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FixResolutionStatus {
    pub resolved_ids: Vec<String>,
    pub still_blocking_ids: Vec<String>,
    pub all_resolved: bool,
}

impl FixResolutionStatus {
    pub fn new() -> Self {
        Self {
            resolved_ids: Vec::new(),
            still_blocking_ids: Vec::new(),
            all_resolved: true,
        }
    }

    pub fn summary(&self) -> String {
        if self.all_resolved {
            format!("All {} fix(es) verified", self.resolved_ids.len())
        } else {
            format!(
                "{} resolved, {} still blocking",
                self.resolved_ids.len(),
                self.still_blocking_ids.len()
            )
        }
    }
}

impl Default for FixResolutionStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl QAStatus {
    pub fn display_summary(&self) -> String {
        if self.is_merge_ready {
            return "✓ QA Approved".to_string();
        }

        if self.needs_escalation {
            return format!(
                "⚠ Escalated: {}",
                self.escalation_reason
                    .as_deref()
                    .unwrap_or("Max retries exceeded")
            );
        }

        let iter_info = format!("{}/{}", self.iteration, self.max_retries);
        let fix_info = if self.pending_fixes > 0 {
            format!(", {} fixes pending", self.pending_fixes)
        } else {
            String::new()
        };

        format!("QA {} [{}{}]", self.state, iter_info, fix_info)
    }
}
