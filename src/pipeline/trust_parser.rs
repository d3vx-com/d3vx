//! Unified Trust Data Parser
//!
//! Provides a single source of truth for parsing canonical trust data from task metadata.
//! Handles both modern (merge_readiness) and legacy (review_summary) formats.

use crate::pipeline::docs_completeness::DocsCompleteness;
use crate::pipeline::merge_gate::MergeReadiness;
use crate::pipeline::qa_loop::QAStatus;
use crate::pipeline::review_summary::ReviewSummary;
use crate::pipeline::validation_summary::ValidationSummary;

#[derive(Debug, Clone)]
pub struct UnifiedTrustData {
    pub merge_readiness: Option<MergeReadiness>,
    pub review_summary: Option<ReviewSummary>,
    pub validation_summary: Option<ValidationSummary>,
    pub docs_completeness: Option<DocsCompleteness>,
    pub qa_status: Option<QAStatus>,
}

impl UnifiedTrustData {
    pub fn from_metadata(metadata: &serde_json::Value) -> Self {
        Self {
            merge_readiness: Self::parse_merge_readiness(metadata),
            review_summary: Self::parse_review_summary(metadata),
            validation_summary: Self::parse_validation_summary(metadata),
            docs_completeness: Self::parse_docs_completeness(metadata),
            qa_status: Self::parse_qa_status(metadata),
        }
    }

    fn parse_merge_readiness(metadata: &serde_json::Value) -> Option<MergeReadiness> {
        metadata
            .get("merge_readiness")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
    }

    fn parse_review_summary(metadata: &serde_json::Value) -> Option<ReviewSummary> {
        metadata
            .get("review_summary")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
    }

    fn parse_validation_summary(metadata: &serde_json::Value) -> Option<ValidationSummary> {
        metadata
            .get("validation_summary")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
    }

    fn parse_docs_completeness(metadata: &serde_json::Value) -> Option<DocsCompleteness> {
        metadata
            .get("docs_completeness")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
    }

    fn parse_qa_status(metadata: &serde_json::Value) -> Option<QAStatus> {
        metadata
            .get("qa_status")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
    }

    pub fn is_merge_ready(&self) -> bool {
        self.merge_readiness
            .as_ref()
            .map(|r| r.ready)
            .unwrap_or_else(|| {
                self.review_summary
                    .as_ref()
                    .map(|r| {
                        !r.merge_blocked
                            && r.status == crate::pipeline::review_summary::ReviewStatus::Approved
                    })
                    .unwrap_or(false)
            })
    }

    pub fn blocking_count(&self) -> usize {
        self.merge_readiness
            .as_ref()
            .map(|r| r.reasons.len())
            .unwrap_or_else(|| {
                self.review_summary
                    .as_ref()
                    .map(|r| r.blocking_findings.len())
                    .unwrap_or(0)
            })
    }

    pub fn qa_iteration(&self) -> u32 {
        self.qa_status.as_ref().map(|s| s.iteration).unwrap_or(0)
    }

    pub fn needs_escalation(&self) -> bool {
        self.qa_status
            .as_ref()
            .map(|s| s.needs_escalation)
            .unwrap_or(false)
    }
}
