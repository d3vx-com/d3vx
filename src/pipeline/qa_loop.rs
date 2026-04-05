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
        // Ensure timer is set — some code paths don't call start_* explicitly.
        if self.current_phase_start.is_none() {
            self.current_phase_start = Some(Instant::now());
        }
        let start = self.current_phase_start.take().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::docs_completeness::{DocsCompleteness, DocsStatus};
    use crate::pipeline::review_gate::GateResult;
    use crate::pipeline::review_summary::{FindingCategory, ReviewFinding, ReviewSeverity};

    fn make_gate_result(blocked: bool, reasons: Vec<BlockingReason>) -> GateResult {
        GateResult {
            blocked,
            reasons,
            warnings: Vec::new(),
            ready: !blocked,
        }
    }

    fn make_review_summary() -> ReviewSummary {
        ReviewSummary::new("task-1".to_string())
    }

    #[test]
    fn test_qa_loop_initial_state() {
        let qa = QALoop::new("task-1".to_string(), QAConfig::default());
        assert_eq!(qa.state, QAState::Pending);
        assert_eq!(qa.current_iteration, 0);
        assert!(qa.can_continue());
    }

    #[test]
    fn test_review_transitions_to_approved() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
        qa.start_review();

        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        assert!(matches!(transition, QATransition::Approved));
        assert_eq!(qa.state, QAState::Approved);
    }

    #[test]
    fn test_review_triggers_fix_loop() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
        qa.start_review();

        let review = make_review_summary();
        let gate = make_gate_result(
            true,
            vec![BlockingReason {
                code: "SECURITY_ISSUE".to_string(),
                message: "Security finding".to_string(),
                category: super::super::review_summary::FindingCategory::Security,
                finding_ids: vec!["f1".to_string()],
            }],
        );

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        assert!(matches!(
            transition,
            QATransition::NeedsFix {
                reasons,
                iteration: 1
            } if reasons.len() == 1
        ));
        assert_eq!(qa.state, QAState::AwaitingFix);
    }

    #[test]
    fn test_max_retries_escalates() {
        let mut qa = QALoop::new(
            "task-1".to_string(),
            QAConfig {
                max_retries: 2,
                require_validation: false,
                auto_fix_on_nonblocking: false,
            },
        );

        for _ in 0..2 {
            qa.start_review();
            let review = make_review_summary();
            let gate = make_gate_result(true, vec![]);
            qa.record_review_result(&review, &gate);
            qa.check_and_transition(&gate);
        }

        assert_eq!(qa.current_iteration, 2);
        assert!(qa.should_escalate());
        assert!(matches!(
            qa.state,
            QAState::AwaitingFix | QAState::Escalated
        ));
    }

    #[test]
    fn test_fix_then_approve() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review1 = make_review_summary();
        let gate1 = make_gate_result(true, vec![]);
        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let review2 = make_review_summary();
        let gate2 = make_gate_result(false, vec![]);
        qa.record_review_result(&review2, &gate2);
        let transition = qa.check_and_transition(&gate2);

        assert!(matches!(transition, QATransition::Approved));
        assert_eq!(qa.history.len(), 3);
    }

    #[test]
    fn test_history_records_duration() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
        qa.start_review();

        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let record = &qa.history[0];
        assert_eq!(record.iteration, 1);
        assert!(record.duration_ms.is_some());
        assert!(!record.blocked);
    }

    #[test]
    fn test_metadata_roundtrip() {
        let qa = QALoop::new("task-1".to_string(), QAConfig::default());
        let inner = qa.to_metadata();
        let metadata = serde_json::json!({ "qa_loop": inner });

        let restored = QALoop::from_metadata("task-1".to_string(), &metadata).unwrap();
        assert_eq!(restored.task_id, qa.task_id);
        assert_eq!(restored.max_retries, qa.max_retries);
    }

    #[test]
    fn test_status_display() {
        let qa = QALoop::new("task-1".to_string(), QAConfig::default());
        let status = qa.current_status();

        assert_eq!(status.state, QAState::Pending);
        assert!(!status.is_merge_ready);

        let display = status.display_summary();
        assert!(display.contains("pending"));
    }

    #[test]
    fn test_blocking_issues_trigger_fix_loop() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();

        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "sec-1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::High,
            title: "SQL Injection vulnerability".to_string(),
            description: "User input not sanitized".to_string(),
            location: None,
            suggestion: Some("Use parameterized queries".to_string()),
            resolved: false,
        });
        review.finalize();

        let gate = GateResult {
            blocked: true,
            reasons: vec![BlockingReason {
                code: "SECURITY_ISSUE".to_string(),
                message: "SQL Injection vulnerability".to_string(),
                category: FindingCategory::Security,
                finding_ids: vec!["sec-1".to_string()],
            }],
            warnings: vec![],
            ready: false,
        };

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        match transition {
            QATransition::NeedsFix { reasons, iteration } => {
                assert_eq!(iteration, 1);
                assert_eq!(reasons.len(), 1);
                assert_eq!(reasons[0].code, "SECURITY_ISSUE");
            }
            _ => panic!("Expected NeedsFix transition"),
        }

        assert_eq!(qa.state, QAState::AwaitingFix);
        assert_eq!(qa.pending_findings.len(), 1);
        assert_eq!(qa.pending_findings[0].id, "sec-1");
    }

    #[test]
    fn test_successful_rereview_unblocks_merge() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Null pointer exception".to_string(),
            description: "Object not initialized".to_string(),
            location: None,
            suggestion: Some("Initialize object before use".to_string()),
            resolved: false,
        });
        review1.finalize();

        let gate1 = GateResult {
            blocked: true,
            reasons: vec![BlockingReason {
                code: "BUG_FOUND".to_string(),
                message: "Bug found".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["bug-1".to_string()],
            }],
            warnings: vec![],
            ready: false,
        };

        qa.record_review_result(&review1, &gate1);
        assert!(matches!(
            qa.check_and_transition(&gate1),
            QATransition::NeedsFix { .. }
        ));
        assert_eq!(qa.state, QAState::AwaitingFix);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug fixed".to_string(),
            description: "Object now properly initialized".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review2.finalize();

        let gate2 = GateResult {
            blocked: false,
            reasons: vec![],
            warnings: vec![],
            ready: true,
        };

        qa.record_review_result(&review2, &gate2);
        let transition = qa.check_and_transition(&gate2);

        assert!(matches!(transition, QATransition::Approved));
        assert_eq!(qa.state, QAState::Approved);
        assert!(qa.current_status().is_merge_ready);
    }

    #[test]
    fn test_fix_mode_sets_appropriate_instruction() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "sec-1".to_string(),
            title: "SQL Injection".to_string(),
            category: "Security".to_string(),
            severity: "High".to_string(),
            suggestion: Some("Use parameterized queries".to_string()),
            created_at_iteration: 1,
        });

        qa.start_fix();
        qa.record_fix_result(None, None);

        assert_eq!(qa.state, QAState::InFix);
        assert_eq!(qa.history.len(), 1);
        assert!(matches!(qa.history[0].phase, QAPhase::Fix));
    }

    #[test]
    fn test_multiple_findings_all_recorded() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();

        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::Critical,
            title: "Auth bypass".to_string(),
            description: "...".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.add_finding(ReviewFinding {
            id: "f2".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Memory leak".to_string(),
            description: "...".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.finalize();

        let gate = GateResult {
            blocked: true,
            reasons: vec![
                BlockingReason {
                    code: "SECURITY".to_string(),
                    message: "Security issue".to_string(),
                    category: FindingCategory::Security,
                    finding_ids: vec!["f1".to_string()],
                },
                BlockingReason {
                    code: "BUG".to_string(),
                    message: "Bug".to_string(),
                    category: FindingCategory::Correctness,
                    finding_ids: vec!["f2".to_string()],
                },
            ],
            warnings: vec![],
            ready: false,
        };

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        assert!(matches!(transition, QATransition::NeedsFix { reasons, .. } if reasons.len() == 2));
        assert_eq!(qa.pending_findings.len(), 2);
    }

    #[test]
    fn test_history_preserves_all_iterations() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        for i in 1..=2 {
            qa.start_review();
            let review = make_review_summary();
            let blocked = i == 1;
            let gate = make_gate_result(blocked, vec![]);
            qa.record_review_result(&review, &gate);
            qa.check_and_transition(&gate);

            if blocked {
                qa.start_fix();
                qa.record_fix_result(None, None);
            }
        }

        assert_eq!(qa.history.len(), 3);
        assert_eq!(qa.history[0].phase, QAPhase::Review);
        assert_eq!(qa.history[1].phase, QAPhase::Fix);
        assert_eq!(qa.history[2].phase, QAPhase::Review);
    }

    #[test]
    fn test_blocked_review_creates_pending_fixes() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::High,
            title: "SQL Injection".to_string(),
            description: "User input not sanitized".to_string(),
            location: None,
            suggestion: Some("Use parameterized queries".to_string()),
            resolved: false,
        });
        review.finalize();

        let gate = make_gate_result(
            true,
            vec![BlockingReason {
                code: "SECURITY".to_string(),
                message: "SQL Injection vulnerability".to_string(),
                category: FindingCategory::Security,
                finding_ids: vec!["f1".to_string()],
            }],
        );

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        assert!(matches!(transition, QATransition::NeedsFix { .. }));
        assert_eq!(qa.state, QAState::AwaitingFix);
        assert_eq!(qa.pending_findings.len(), 1);
        assert_eq!(qa.pending_findings[0].id, "f1");
        assert_eq!(qa.pending_findings[0].created_at_iteration, 1);
    }

    #[test]
    fn test_fix_mode_picks_up_pending_findings() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "f1".to_string(),
            title: "SQL Injection".to_string(),
            category: "Security".to_string(),
            severity: "High".to_string(),
            suggestion: Some("Use parameterized queries".to_string()),
            created_at_iteration: 1,
        });

        qa.start_fix();
        assert_eq!(qa.state, QAState::InFix);
        assert_eq!(qa.pending_findings.len(), 1);

        let pending_ids = qa.get_pending_finding_ids();
        assert_eq!(pending_ids, vec!["f1"]);
    }

    #[test]
    fn test_rereview_clears_resolved_findings() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "f1".to_string(),
            title: "Bug fixed".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });

        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review.finalize();

        let gate = make_gate_result(false, vec![]);
        let transition = qa.handle_rereview_result(&review, &gate);

        assert!(matches!(transition, QATransition::Approved));
        assert_eq!(qa.state, QAState::Approved);
        assert!(qa.pending_findings.is_empty());
    }

    #[test]
    fn test_repeated_failures_escalate_cleanly() {
        let mut qa = QALoop::new(
            "task-1".to_string(),
            QAConfig {
                max_retries: 3,
                require_validation: false,
                auto_fix_on_nonblocking: false,
            },
        );

        let gate_blocked = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug persists".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["f1".to_string()],
            }],
        );

        for iteration in 1..=3 {
            qa.start_review();
            let review = make_review_summary();
            qa.record_review_result(&review, &gate_blocked);
            qa.check_and_transition(&gate_blocked);

            if iteration < 3 {
                assert_eq!(qa.state, QAState::AwaitingFix);
                qa.start_fix();
                qa.record_fix_result(None, None);
            }
        }

        assert!(qa.should_escalate());
        assert_eq!(qa.state, QAState::Escalated);
        assert!(qa.escalation_reason.is_some());
        assert!(qa.current_status().needs_escalation);

        let status = qa.current_status();
        let display = status.display_summary();
        assert!(display.contains("Escalated"));
    }

    #[test]
    fn test_full_fix_loop_approve_after_retry() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug in code".to_string(),
            description: "Bug description".to_string(),
            location: None,
            suggestion: Some("Fix the bug".to_string()),
            resolved: false,
        });
        review1.finalize();

        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug in code".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["f1".to_string()],
            }],
        );

        qa.record_review_result(&review1, &gate1);
        let t1 = qa.check_and_transition(&gate1);
        assert!(matches!(t1, QATransition::NeedsFix { .. }));
        assert_eq!(qa.current_iteration, 1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug fixed".to_string(),
            description: "Fixed now".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review2.finalize();

        let gate2 = make_gate_result(false, vec![]);
        qa.record_review_result(&review2, &gate2);
        let t2 = qa.check_and_transition(&gate2);

        assert!(matches!(t2, QATransition::Approved));
        assert_eq!(qa.state, QAState::Approved);
        assert!(qa.pending_findings.is_empty());
        assert_eq!(qa.history.len(), 3);
    }

    #[test]
    fn test_fix_resolution_verification() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "f1".to_string(),
            title: "Bug 1".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });
        qa.add_pending_finding(PendingFinding {
            id: "f2".to_string(),
            title: "Bug 2".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });

        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug 1 fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review.add_finding(ReviewFinding {
            id: "f2".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug 2 still present".to_string(),
            description: "Still broken".to_string(),
            location: None,
            suggestion: Some("Fix it".to_string()),
            resolved: false,
        });

        let resolution = qa.verify_fix_resolution(&review);

        assert!(!resolution.all_resolved);
        assert_eq!(resolution.resolved_ids, vec!["f1"]);
        assert_eq!(resolution.still_blocking_ids, vec!["f2"]);
    }

    #[test]
    fn test_validation_update_influences_state() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_fix();

        let mut validation = ValidationSummary::new(Some("task-1".to_string()));
        validation.confidence = super::super::validation_summary::Confidence::Low;
        validation.failed = 2;
        validation.total = 3;

        qa.update_from_validation(&validation);

        assert_eq!(qa.state, QAState::AwaitingFix);
        assert!(qa.last_validation.is_some());
    }

    #[test]
    fn test_pending_findings_no_duplicates() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "f1".to_string(),
            title: "Bug 1".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });

        qa.add_pending_finding(PendingFinding {
            id: "f1".to_string(),
            title: "Bug 1 again".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });

        assert_eq!(qa.pending_findings.len(), 1);
    }

    #[test]
    fn test_fix_resolution_status_summary() {
        let mut status = FixResolutionStatus::new();
        status.resolved_ids = vec!["f1".to_string(), "f2".to_string()];
        status.all_resolved = true;

        assert!(status.summary().contains("2 fix"));

        status.still_blocking_ids = vec!["f3".to_string()];
        status.all_resolved = false;

        let partial_summary = status.summary();
        assert!(partial_summary.contains("resolved"));
        assert!(partial_summary.contains("still blocking"));
    }

    #[test]
    fn test_record_review_produces_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        assert!(qa.last_merge_readiness.is_none());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);

        qa.record_review_result(&review, &gate);

        assert!(qa.last_merge_readiness.is_some());
        let readiness = qa.last_merge_readiness.as_ref().unwrap();
        assert!(readiness.ready);
    }

    #[test]
    fn test_record_fix_updates_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug found".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec![],
            }],
        );
        qa.record_review_result(&review, &gate);

        let first_readiness = qa.last_merge_readiness.clone().unwrap();
        assert!(!first_readiness.ready);

        qa.start_fix();
        qa.record_fix_result(None, None);

        let updated_readiness = qa.last_merge_readiness.as_ref().unwrap();
        assert!(!updated_readiness.ready);
    }

    #[test]
    fn test_get_merge_readiness_returns_stored() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let stored = qa.get_merge_readiness();
        assert!(stored.ready);
        assert_eq!(stored.reasons.len(), 0);
    }

    #[test]
    fn test_to_metadata_includes_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let metadata = qa.to_metadata();

        assert!(metadata.get("merge_readiness").is_some());
        let readiness = metadata.get("merge_readiness").unwrap();
        assert!(readiness.get("ready").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_qa_loop_persists_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let metadata = qa.to_metadata();
        let serialized = serde_json::to_string(&metadata).unwrap();

        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.get("merge_readiness").is_some());
    }

    #[test]
    fn test_blocked_review_enters_fix_loop() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "sec-1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::Critical,
            title: "SQL Injection".to_string(),
            description: "User input not sanitized".to_string(),
            location: None,
            suggestion: Some("Use parameterized queries".to_string()),
            resolved: false,
        });
        review.finalize();

        let gate = make_gate_result(
            true,
            vec![BlockingReason {
                code: "SECURITY".to_string(),
                message: "Critical security issue".to_string(),
                category: FindingCategory::Security,
                finding_ids: vec!["sec-1".to_string()],
            }],
        );

        qa.record_review_result(&review, &gate);
        let transition = qa.check_and_transition(&gate);

        assert!(matches!(transition, QATransition::NeedsFix { .. }));
        assert_eq!(qa.state, QAState::AwaitingFix);
        assert_eq!(qa.pending_findings.len(), 1);
        assert_eq!(qa.pending_findings[0].id, "sec-1");
        assert_eq!(qa.current_iteration, 1);
    }

    #[test]
    fn test_successful_fix_clears_blockers() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Null pointer".to_string(),
            description: "Potential null pointer".to_string(),
            location: None,
            suggestion: Some("Add null check".to_string()),
            resolved: false,
        });
        review1.finalize();

        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug found".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["bug-1".to_string()],
            }],
        );

        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        assert_eq!(qa.pending_findings.len(), 1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Null pointer fixed".to_string(),
            description: "Now safe".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review2.finalize();

        let gate2 = make_gate_result(false, vec![]);
        let transition = qa.handle_rereview_result(&review2, &gate2);

        assert!(matches!(transition, QATransition::Approved));
        assert_eq!(qa.state, QAState::Approved);
        assert!(qa.pending_findings.is_empty());
    }

    #[test]
    fn test_unresolved_blockers_keep_merge_blocked() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug still present".to_string(),
            description: "Not fixed".to_string(),
            location: None,
            suggestion: Some("Fix it properly".to_string()),
            resolved: false,
        });
        review1.finalize();

        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug not fixed".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["bug-1".to_string()],
            }],
        );

        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        assert_eq!(qa.pending_findings.len(), 1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug still present".to_string(),
            description: "Not fixed".to_string(),
            location: None,
            suggestion: Some("Fix it properly".to_string()),
            resolved: false,
        });
        review2.finalize();

        let gate2 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug not fixed".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["bug-1".to_string()],
            }],
        );
        let transition = qa.handle_rereview_result(&review2, &gate2);

        assert!(matches!(transition, QATransition::NeedsFix { .. }));
        assert_eq!(qa.state, QAState::AwaitingFix);
        assert_eq!(qa.pending_findings.len(), 1);
    }

    #[test]
    fn test_repeated_failures_escalate() {
        let mut qa = QALoop::new(
            "task-1".to_string(),
            QAConfig {
                max_retries: 3,
                require_validation: false,
                auto_fix_on_nonblocking: false,
            },
        );

        let gate_blocked = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug persists".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["bug-1".to_string()],
            }],
        );

        for iteration in 1..=3 {
            qa.start_review();
            let mut review = make_review_summary();
            review.add_finding(ReviewFinding {
                id: "bug-1".to_string(),
                category: FindingCategory::Correctness,
                severity: ReviewSeverity::High,
                title: "Bug".to_string(),
                description: "Not fixed".to_string(),
                location: None,
                suggestion: None,
                resolved: false,
            });
            review.finalize();

            qa.record_review_result(&review, &gate_blocked);
            let transition = qa.check_and_transition(&gate_blocked);

            assert_eq!(qa.current_iteration, iteration as u32);

            if iteration < 3 {
                assert_eq!(qa.state, QAState::AwaitingFix);
                qa.start_fix();
                qa.record_fix_result(None, None);
            }
        }

        assert!(qa.should_escalate());
        assert_eq!(qa.state, QAState::Escalated);
        assert!(qa.escalation_reason.is_some());

        let status = qa.current_status();
        assert!(status.needs_escalation);
    }

    #[test]
    fn test_fix_resolution_tracked_in_history() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "fix-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Issue".to_string(),
            description: "Issue".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review1.finalize();
        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["fix-1".to_string()],
            }],
        );
        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "fix-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review2.finalize();
        let gate2 = make_gate_result(false, vec![]);
        qa.handle_rereview_result(&review2, &gate2);

        let records_with_resolution: Vec<_> = qa
            .history
            .iter()
            .filter(|r| !r.resolved_finding_ids.is_empty() || !r.still_blocking_ids.is_empty())
            .collect();
        assert!(!records_with_resolution.is_empty());
    }

    #[test]
    fn test_validation_blocking_participates_in_qa() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        qa.start_fix();
        let mut validation = ValidationSummary::new(Some("task-1".to_string()));
        validation.confidence = super::super::validation_summary::Confidence::Low;
        validation.total = 5;
        validation.passed = 2;
        validation.failed = 3;
        qa.record_fix_result(Some(&validation), None);

        let readiness = qa.evaluate_merge_readiness();
        assert!(!readiness.ready);
        assert!(readiness
            .reasons
            .iter()
            .any(|r| matches!(r.source, super::super::merge_gate::MergeSource::Validation)));
    }

    #[test]
    fn test_qa_status_display_summary() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        let status = qa.current_status();
        assert!(status.display_summary().contains("pending"));

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(true, vec![]);
        qa.record_review_result(&review, &gate);
        qa.check_and_transition(&gate);

        let blocked_status = qa.current_status();
        assert!(
            blocked_status.display_summary().contains("AwaitingFix")
                || blocked_status.display_summary().contains("fix")
        );
    }

    #[test]
    fn test_handle_rereview_result_records_resolution() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "partial-fix".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Issue".to_string(),
            description: "Issue".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review1.finalize();
        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["partial-fix".to_string()],
            }],
        );
        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "partial-fix".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Partially fixed".to_string(),
            description: "Partially fixed".to_string(),
            location: None,
            suggestion: Some("Complete the fix".to_string()),
            resolved: false,
        });
        review2.finalize();
        let gate2 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Still broken".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["partial-fix".to_string()],
            }],
        );

        let transition = qa.handle_rereview_result(&review2, &gate2);

        assert!(matches!(transition, QATransition::NeedsFix { .. }));
        let last_record = qa.history.last().unwrap();
        assert!(!last_record.still_blocking_ids.is_empty());
    }

    #[test]
    fn test_clear_pending_finding() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.add_pending_finding(PendingFinding {
            id: "to-clear".to_string(),
            title: "Issue".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });
        qa.add_pending_finding(PendingFinding {
            id: "to-keep".to_string(),
            title: "Issue 2".to_string(),
            category: "Correctness".to_string(),
            severity: "High".to_string(),
            suggestion: None,
            created_at_iteration: 1,
        });

        assert_eq!(qa.pending_findings.len(), 2);

        qa.clear_pending_finding("to-clear");

        assert_eq!(qa.pending_findings.len(), 1);
        assert_eq!(qa.pending_findings[0].id, "to-keep");
    }

    #[test]
    fn test_full_qa_cycle_with_multiple_findings() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review1 = make_review_summary();
        review1.add_finding(ReviewFinding {
            id: "issue-a".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Issue A".to_string(),
            description: "Issue A".to_string(),
            location: None,
            suggestion: Some("Fix A".to_string()),
            resolved: false,
        });
        review1.add_finding(ReviewFinding {
            id: "issue-b".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::Medium,
            title: "Issue B".to_string(),
            description: "Issue B".to_string(),
            location: None,
            suggestion: Some("Fix B".to_string()),
            resolved: false,
        });
        review1.finalize();

        let gate1 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Two issues".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["issue-a".to_string(), "issue-b".to_string()],
            }],
        );

        qa.record_review_result(&review1, &gate1);
        qa.check_and_transition(&gate1);

        assert_eq!(qa.pending_findings.len(), 2);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review2 = make_review_summary();
        review2.add_finding(ReviewFinding {
            id: "issue-a".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Issue A fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review2.add_finding(ReviewFinding {
            id: "issue-b".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::Medium,
            title: "Issue B still present".to_string(),
            description: "Not fixed".to_string(),
            location: None,
            suggestion: Some("Fix B".to_string()),
            resolved: false,
        });
        review2.finalize();

        let gate2 = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Issue B not fixed".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["issue-b".to_string()],
            }],
        );

        let transition = qa.handle_rereview_result(&review2, &gate2);

        assert!(matches!(transition, QATransition::NeedsFix { .. }));
        assert_eq!(qa.pending_findings.len(), 1);
        assert_eq!(qa.pending_findings[0].id, "issue-b");

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut review3 = make_review_summary();
        review3.add_finding(ReviewFinding {
            id: "issue-a".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Issue A fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review3.add_finding(ReviewFinding {
            id: "issue-b".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::Medium,
            title: "Issue B fixed".to_string(),
            description: "Fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        review3.finalize();

        let gate3 = make_gate_result(false, vec![]);
        let final_transition = qa.handle_rereview_result(&review3, &gate3);

        assert!(matches!(final_transition, QATransition::Approved));
        assert!(qa.pending_findings.is_empty());
        assert_eq!(qa.current_iteration, 1);
    }

    #[test]
    fn test_record_docs_result_updates_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let docs_before = DocsCompleteness::not_evaluated(Some("task-1".to_string()));
        qa.record_docs_result(&docs_before);

        assert!(qa.last_merge_readiness.is_some());
        assert!(qa.last_docs.is_some());
    }

    #[test]
    fn test_record_docs_result_updates_stale_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(
            true,
            vec![BlockingReason {
                code: "BUG".to_string(),
                message: "Bug found".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec![],
            }],
        );
        qa.record_review_result(&review, &gate);

        let readiness_before = qa.last_merge_readiness.clone().unwrap();
        assert!(
            !readiness_before.ready,
            "Should be blocked by review findings"
        );

        let docs = DocsCompleteness {
            task_id: Some("task-1".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec!["README.md".to_string(), "api.md".to_string()],
            evaluated_at: None,
        };
        qa.record_docs_result(&docs);

        let readiness_after = qa.last_merge_readiness.clone().unwrap();
        assert!(!readiness_after.ready, "Still blocked by review findings");
    }

    #[test]
    fn test_record_docs_result_evaluates_canonically() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let docs = DocsCompleteness::not_evaluated(Some("task-1".to_string()));
        qa.record_docs_result(&docs);

        let from_last = qa.last_merge_readiness.clone().unwrap();
        let from_evaluate = qa.evaluate_merge_readiness();

        assert_eq!(from_last.ready, from_evaluate.ready);
        assert_eq!(from_last.reasons.len(), from_evaluate.reasons.len());
    }

    #[test]
    fn test_to_metadata_after_docs_agrees_with_top_level() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let docs = DocsCompleteness {
            task_id: Some("task-1".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec!["README.md".to_string()],
            evaluated_at: None,
        };
        qa.record_docs_result(&docs);

        let metadata = qa.to_metadata();
        let embedded_readiness = metadata.get("merge_readiness").unwrap();
        let top_level_readiness = qa.evaluate_merge_readiness();
        let top_level_json = serde_json::to_value(&top_level_readiness).unwrap();

        assert_eq!(
            embedded_readiness.get("ready").unwrap(),
            top_level_json.get("ready").unwrap(),
            "Embedded merge_readiness in qa_loop should match top-level"
        );
    }

    #[test]
    fn test_qa_loop_roundtrip_after_docs_updates_merge_readiness() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let docs = DocsCompleteness {
            task_id: Some("task-1".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec!["README.md".to_string()],
            evaluated_at: None,
        };
        qa.record_docs_result(&docs);

        let metadata = qa.to_metadata();
        let wrapper = serde_json::json!({ "qa_loop": metadata });
        let serialized = serde_json::to_string(&wrapper).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        let restored = QALoop::from_metadata("task-1".to_string(), &deserialized).unwrap();
        let restored_readiness = restored.get_merge_readiness();
        let original_readiness = qa.get_merge_readiness();

        assert_eq!(
            restored_readiness.ready, original_readiness.ready,
            "Restored qa_loop should have same merge readiness"
        );
    }

    #[test]
    fn test_docs_result_uses_all_phase_states() {
        let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

        qa.start_review();
        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug".to_string(),
            description: "Bug desc".to_string(),
            location: None,
            suggestion: Some("Fix it".to_string()),
            resolved: false,
        });
        review.finalize();
        let gate = make_gate_result(false, vec![]);
        qa.record_review_result(&review, &gate);

        let mut validation = ValidationSummary::new(Some("task-1".to_string()));
        validation.passed = 5;
        validation.total = 10;
        validation.failed = 5;
        qa.update_from_validation(&validation);

        let docs = DocsCompleteness {
            task_id: Some("task-1".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec!["README.md".to_string()],
            evaluated_at: None,
        };
        qa.record_docs_result(&docs);

        let readiness = qa.get_merge_readiness();
        assert!(qa.last_review.is_some());
        assert!(qa.last_validation.is_some());
        assert!(qa.last_docs.is_some());
        assert!(qa.last_merge_readiness.is_some());
        assert!(readiness.signals.validation.is_some());
    }
}
