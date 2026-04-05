//! Review phase handler with QA loop integration

use async_trait::async_trait;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;
use crate::pipeline::qa_loop::{PendingFinding, QAConfig, QALoop, QATransition};
use crate::pipeline::review_gate::ReviewGate;
use crate::pipeline::review_summary::{
    FindingCategory, ReviewFinding, ReviewSeverity, ReviewSummary,
};

fn parse_findings(output: &str, task_id: &str) -> Vec<ReviewFinding> {
    let mut findings = Vec::new();

    if let Some(json_findings) = extract_json_findings(output, task_id) {
        findings.extend(json_findings);
    }

    if findings.is_empty() {
        let markdown_findings = extract_markdown_findings(output, task_id);
        findings.extend(markdown_findings);
    }

    findings
}

fn extract_json_findings(output: &str, task_id: &str) -> Option<Vec<ReviewFinding>> {
    let json_regex = Regex::new(r#"\{"findings"\s*:\s*\["#).ok()?;
    if !json_regex.is_match(output) {
        return None;
    }

    let start_idx = output.find('[')?;
    let end_idx = find_matching_bracket(output, start_idx)?;

    let json_str = &output[start_idx..=end_idx];
    let parsed: serde_json::Result<Vec<serde_json::Value>> = serde_json::from_str(json_str);

    if let Ok(values) = parsed {
        let findings: Vec<ReviewFinding> = values
            .iter()
            .filter_map(|v| json_value_to_finding(v, task_id))
            .collect();

        if !findings.is_empty() {
            return Some(findings);
        }
    }

    let alt_json_regex = Regex::new(r#"\[[\s\S]*\{[\s\S]*"severity"[\s\S]*"[\s\S]*\]"#).ok()?;
    if let Some(captures) = alt_json_regex.captures(output) {
        let json_str = captures.get(0)?.as_str();
        if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
            let findings: Vec<ReviewFinding> = values
                .iter()
                .filter_map(|v| json_value_to_finding(v, task_id))
                .collect();
            if !findings.is_empty() {
                return Some(findings);
            }
        }
    }

    None
}

fn json_value_to_finding(value: &serde_json::Value, task_id: &str) -> Option<ReviewFinding> {
    let severity = value
        .get("severity")
        .and_then(|v| v.as_str())
        .and_then(parse_severity)
        .unwrap_or(ReviewSeverity::Medium);

    let category = value
        .get("category")
        .and_then(|v| v.as_str())
        .and_then(parse_category)
        .unwrap_or(FindingCategory::Correctness);

    let title = value.get("title").and_then(|v| v.as_str())?.to_string();
    let description = value
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| title.clone());

    let location = value.get("location").and_then(|v| {
        let file = v.get("file")?.as_str()?.to_string();
        let line = v.get("line").and_then(|l| l.as_u64()).map(|n| n as u32);
        let column = v.get("column").and_then(|c| c.as_u64()).map(|n| n as u32);
        Some(crate::pipeline::review_summary::FindingLocation { file, line, column })
    });

    let suggestion = value
        .get("suggestion")
        .and_then(|v| v.as_str())
        .map(String::from);
    let resolved = value
        .get("resolved")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Some(ReviewFinding {
        id: format!("{}-finding-{}", task_id, uuid::Uuid::new_v4().as_simple()),
        category,
        severity,
        title,
        description,
        location,
        suggestion,
        resolved,
    })
}

fn extract_markdown_findings(output: &str, task_id: &str) -> Vec<ReviewFinding> {
    let mut findings = Vec::new();

    let severity_patterns = [
        (
            r"(?i)!!!?\s+(?:\[[\w\s]+\])?\s*(.+?)(?:\n|$)",
            ReviewSeverity::Critical,
        ),
        (
            r"(?i)!!\s+(?:\[[\w\s]+\])?\s*(.+?)(?:\n|$)",
            ReviewSeverity::High,
        ),
        (
            r"(?i)!\s+(?:\[[\w\s]+\])?\s*(.+?)(?:\n|$)",
            ReviewSeverity::Medium,
        ),
        (
            r"(?i)o\s+(?:\[[\w\s]+\])?\s*(.+?)(?:\n|$)",
            ReviewSeverity::Low,
        ),
        (
            r"(?i)\[CRITICAL\]\s*(.+?)(?:\n|$)",
            ReviewSeverity::Critical,
        ),
        (r"(?i)\[HIGH\]\s*(.+?)(?:\n|$)", ReviewSeverity::High),
        (r"(?i)\[MEDIUM\]\s*(.+?)(?:\n|$)", ReviewSeverity::Medium),
        (r"(?i)\[LOW\]\s*(.+?)(?:\n|$)", ReviewSeverity::Low),
    ];

    let category_patterns = [
        (r"(?i)\[SEC(?:URITY)?\]", FindingCategory::Security),
        (r"(?i)\[CORR(?:ECTNESS)?\]", FindingCategory::Correctness),
        (r"(?i)\[PERF(?:ORMANCE)?\]", FindingCategory::Performance),
        (
            r"(?i)\[MAIN(?:TAINABILITY)?\]",
            FindingCategory::Maintainability,
        ),
        (r"(?i)\[COV(?:ERAGE)?\]", FindingCategory::Coverage),
        (r"(?i)\[BRK(?:EAKING)?\]", FindingCategory::Breaking),
        (r"(?i)\[RISK\]", FindingCategory::Risk),
        (r"(?i)\[DOC(?:S)?\]", FindingCategory::Documentation),
    ];

    for (pattern, severity) in &severity_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for caps in re.captures_iter(output) {
                let full_match = caps.get(0).unwrap().as_str();
                let title = caps.get(1).unwrap().as_str().trim().to_string();

                let mut category = FindingCategory::Correctness;
                for (cat_pattern, cat) in &category_patterns {
                    if let Ok(cat_re) = Regex::new(cat_pattern) {
                        if cat_re.is_match(full_match) {
                            category = *cat;
                            break;
                        }
                    }
                }

                let description = extract_description_for_finding(output, &title);
                let location = extract_location_from_context(output, &title);
                let suggestion = extract_suggestion_for_finding(output, &title);

                findings.push(ReviewFinding {
                    id: format!("{}-{}", task_id, uuid::Uuid::new_v4().as_simple()),
                    category,
                    severity: *severity,
                    title,
                    description,
                    location,
                    suggestion,
                    resolved: false,
                });
            }
        }
    }

    findings
}

fn extract_description_for_finding(output: &str, title: &str) -> String {
    let title_lower = title.to_lowercase();
    let lines: Vec<&str> = output.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(&title_lower) {
            if i + 1 < lines.len() {
                let next_line = lines[i + 1].trim();
                if !next_line.is_empty()
                    && !next_line.starts_with("!!")
                    && !next_line.starts_with("!")
                    && !next_line.starts_with("[")
                {
                    return next_line.to_string();
                }
            }
        }
    }

    String::new()
}

fn extract_location_from_context(
    output: &str,
    _title: &str,
) -> Option<crate::pipeline::review_summary::FindingLocation> {
    let file_pattern = Regex::new(r"([^\s:]+(?:\.[a-zA-Z]+)?):(\d+)").ok()?;

    for caps in file_pattern.captures_iter(output) {
        if let (Some(file), Some(line)) = (caps.get(1), caps.get(2)) {
            let file_str = file.as_str();
            if !file_str.starts_with("http") && !file_str.contains("...") && file_str.contains('.')
            {
                return Some(crate::pipeline::review_summary::FindingLocation {
                    file: file_str.to_string(),
                    line: line.as_str().parse().ok(),
                    column: None,
                });
            }
        }
    }

    None
}

fn extract_suggestion_for_finding(output: &str, title: &str) -> Option<String> {
    let patterns = [
        r"(?i)suggest(?:ion|ed fix|ed fix):\s*(.+)",
        r"(?i)fix:\s*(.+)",
        r"(?i)should:\s*(.+)",
        r"(?i)consider:\s*(.+)",
    ];

    let title_lower = title.to_lowercase();
    let lines: Vec<&str> = output.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(&title_lower) {
            for p in &patterns {
                if let Ok(re) = Regex::new(p) {
                    if let Some(caps) = re.captures(line) {
                        if let Some(m) = caps.get(1) {
                            return Some(m.as_str().trim().to_string());
                        }
                    }
                }
            }

            if i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if next.starts_with("suggest")
                    || next.starts_with("fix:")
                    || next.starts_with("should:")
                    || next.starts_with("consider:")
                {
                    return Some(next.splitn(2, ':').nth(1)?.trim().to_string());
                }
            }
        }
    }

    None
}

fn parse_severity(s: &str) -> Option<ReviewSeverity> {
    match s.to_lowercase().as_str() {
        "critical" | "crit" | "blocker" => Some(ReviewSeverity::Critical),
        "high" | "error" => Some(ReviewSeverity::High),
        "medium" | "warning" | "warn" => Some(ReviewSeverity::Medium),
        "low" | "info" | "information" | "suggestion" => Some(ReviewSeverity::Low),
        _ => None,
    }
}

fn parse_category(s: &str) -> Option<FindingCategory> {
    match s.to_lowercase().replace('_', "").as_str() {
        "correctness" | "correct" | "bug" | "defect" => Some(FindingCategory::Correctness),
        "security" | "sec" | "vulnerability" | "auth" => Some(FindingCategory::Security),
        "performance" | "perf" | "optimization" => Some(FindingCategory::Performance),
        "maintainability" | "maintain" | "style" | "codequality" => {
            Some(FindingCategory::Maintainability)
        }
        "coverage" | "tests" | "testing" | "test" => Some(FindingCategory::Coverage),
        "breaking" | "breakingchange" | "api" => Some(FindingCategory::Breaking),
        "risk" | "risky" | "concern" => Some(FindingCategory::Risk),
        "documentation" | "docs" | "doc" => Some(FindingCategory::Documentation),
        _ => None,
    }
}

fn find_matching_bracket(s: &str, start: usize) -> Option<usize> {
    let mut depth = 0;
    let chars: Vec<char> = s[start..].chars().collect();

    for (i, c) in chars.iter().enumerate() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn determine_review_outcome(output: &str, findings: &[ReviewFinding]) -> (bool, String) {
    let blocking_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity.blocks_merge() && !f.resolved)
        .collect();

    if !blocking_findings.is_empty() {
        let summaries: Vec<_> = blocking_findings
            .iter()
            .map(|f| format!("{} ({:?})", f.title, f.severity))
            .collect();
        return (false, format!("Blocking: {}", summaries.join(", ")));
    }

    if findings.is_empty() {
        if output.contains("REVIEW: APPROVED") || output.contains("REVIEW: FIXED") {
            return (true, "Review approved by agent".to_string());
        }
        if output.contains("REVIEW: REJECTED") {
            return (false, "Review rejected by agent".to_string());
        }
        if output.to_lowercase().contains("looks good")
            || output.to_lowercase().contains("lgtm")
            || output.to_lowercase().contains("no issues")
            || output.to_lowercase().contains("all checks pass")
        {
            return (true, "Implicit approval from output".to_string());
        }
        return (false, "No findings and no approval signal".to_string());
    }

    let non_blocking_findings: Vec<_> = findings
        .iter()
        .filter(|f| !f.severity.blocks_merge())
        .collect();

    if non_blocking_findings.is_empty() {
        return (true, "All findings resolved".to_string());
    }

    (
        true,
        format!("{} non-blocking finding(s)", non_blocking_findings.len()),
    )
}

pub fn generate_review_rationale(findings: &[ReviewFinding]) -> String {
    if findings.is_empty() {
        return "No issues found - ready to merge".to_string();
    }

    let blocking: Vec<_> = findings
        .iter()
        .filter(|f| f.severity.blocks_merge() && !f.resolved)
        .collect();

    if !blocking.is_empty() {
        return format!(
            "{} blocking issue(s) must be resolved before merge",
            blocking.len()
        );
    }

    let non_blocking: Vec<_> = findings
        .iter()
        .filter(|f| !f.severity.blocks_merge())
        .collect();

    if !non_blocking.is_empty() {
        return format!(
            "{} non-blocking suggestion(s) - ready to merge",
            non_blocking.len()
        );
    }

    "All findings resolved - ready to merge".to_string()
}

/// Review phase handler with QA loop support
pub struct ReviewHandler {
    config: QAConfig,
}

impl ReviewHandler {
    pub fn new() -> Self {
        Self {
            config: QAConfig::default(),
        }
    }

    pub fn with_config(config: QAConfig) -> Self {
        Self { config }
    }

    pub(crate) fn generate_instruction(&self, context: &PhaseContext) -> String {
        prompts::build_phase_instruction(
            Phase::Review,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        )
    }

    fn generate_fix_instruction(
        &self,
        context: &PhaseContext,
        findings: &[ReviewFinding],
    ) -> String {
        let findings_text = findings
            .iter()
            .map(|f| {
                format!(
                    "- [{:?}] {:?}: {}{}",
                    f.severity,
                    f.category,
                    f.title,
                    f.suggestion
                        .as_ref()
                        .map(|s| format!("\n  Suggestion: {}", s))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "{}\n\n## Required Fixes\n\nThe following issues must be resolved:\n\n{}\n\nAddress each finding and verify the fix works correctly.",
            self.generate_instruction(context),
            findings_text
        )
    }

    fn load_qa_loop(&self, task: &Task) -> QALoop {
        let metadata = &task.metadata;

        if let Some(qa) = QALoop::from_metadata(task.id.clone(), metadata) {
            info!(task_id = %task.id, iteration = qa.iteration(), "Resuming QA loop");
            qa
        } else {
            info!(task_id = %task.id, "Starting new QA loop");
            QALoop::new(task.id.clone(), self.config.clone())
        }
    }

    async fn run_review(
        &self,
        agent: &Arc<AgentLoop>,
        instruction: &str,
        task_id: &str,
    ) -> Result<(String, ReviewSummary), PhaseError> {
        let patch_path = Path::new(".d3vx").join(format!("draft-{}.patch", task_id));
        let has_draft = patch_path.exists();

        let mut review = ReviewSummary::new(task_id.to_string());
        review.requested_at = Some(chrono::Utc::now().to_rfc3339());

        agent.clear_history().await;

        if has_draft {
            let patch_content = fs::read_to_string(&patch_path).unwrap_or_default();
            agent
                .add_user_message(&format!(
                    "{}\n\n## Current Draft Patch\n```diff\n{}\n```",
                    instruction, patch_content
                ))
                .await;
        } else {
            agent.add_user_message(instruction).await;
        }

        let result = agent.run().await?;
        let output = result.text.clone();

        let findings = parse_findings(&output, task_id);
        let has_findings = !findings.is_empty();

        if has_findings {
            for finding in findings {
                review.add_finding(finding);
            }
        }

        let (approved, outcome_reason) = determine_review_outcome(&output, &review.findings);

        if has_findings && approved {
            for finding in &mut review.findings {
                finding.resolved = true;
            }
        }

        if !has_findings {
            review.add_finding(ReviewFinding {
                id: format!("review-{}", task_id),
                category: FindingCategory::Correctness,
                severity: if approved {
                    ReviewSeverity::Low
                } else {
                    ReviewSeverity::High
                },
                title: if approved {
                    "Review approved".to_string()
                } else {
                    "Review not approved".to_string()
                },
                description: outcome_reason.clone(),
                location: None,
                suggestion: None,
                resolved: approved,
            });
        }

        review.summary_text = Some(generate_review_rationale(&review.findings));
        review.finalize();

        Ok((output, review))
    }
}

impl Default for ReviewHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ReviewHandler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
        }
    }
}

#[async_trait]
impl PhaseHandler for ReviewHandler {
    fn phase(&self) -> Phase {
        Phase::Review
    }

    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError> {
        self.can_execute(task)?;

        let mut qa_loop = self.load_qa_loop(task);
        let gate = ReviewGate::with_defaults();

        let Some(agent) = agent else {
            let mut review = ReviewSummary::new(task.id.clone());
            review.requested_at = Some(chrono::Utc::now().to_rfc3339());
            let metadata = serde_json::json!({
                "review_summary": review,
                "qa_loop": qa_loop.to_metadata(),
            });
            return Ok(
                PhaseResult::success("Review phase prepared (dry-run)").with_metadata(metadata)
            );
        };

        let instruction = if qa_loop.state == crate::pipeline::qa_loop::QAState::AwaitingFix {
            let blocking_findings: Vec<ReviewFinding> = qa_loop
                .pending_findings
                .iter()
                .map(|pf| ReviewFinding {
                    id: pf.id.clone(),
                    category: serde_json::from_str(&format!("\"{}\"", pf.category))
                        .unwrap_or(FindingCategory::Correctness),
                    severity: serde_json::from_str(&format!("\"{}\"", pf.severity))
                        .unwrap_or(ReviewSeverity::High),
                    title: pf.title.clone(),
                    description: pf.suggestion.clone().unwrap_or_default(),
                    location: None,
                    suggestion: pf.suggestion.clone(),
                    resolved: false,
                })
                .collect();
            self.generate_fix_instruction(context, &blocking_findings)
        } else {
            self.generate_instruction(context)
        };

        let is_rereview = matches!(
            qa_loop.state,
            crate::pipeline::qa_loop::QAState::AwaitingFix
                | crate::pipeline::qa_loop::QAState::InFix
        );
        if is_rereview {
            qa_loop.start_rereview();
            info!(task_id = %task.id, iteration = qa_loop.iteration(), "Starting re-review");
        } else {
            qa_loop.start_review();
            info!(task_id = %task.id, iteration = qa_loop.iteration(), "Starting review");
        }

        let (output, mut review) = self.run_review(&agent, &instruction, &task.id).await?;

        let gate_result = gate.evaluate(&review);

        qa_loop.record_review_result(&review, &gate_result);

        let transition = if is_rereview {
            qa_loop.handle_rereview_result(&review, &gate_result)
        } else {
            qa_loop.check_and_transition(&gate_result)
        };

        match &transition {
            QATransition::Approved => {
                info!(task_id = %task.id, "Review approved - merge ready");
                review.merge_blocked = false;
            }
            QATransition::NeedsFix { reasons, iteration } => {
                warn!(
                    task_id = %task.id,
                    iteration = iteration,
                    blockers = reasons.len(),
                    "Review blocked - fixes needed"
                );

                let blocking_findings: Vec<_> = review
                    .findings
                    .iter()
                    .filter(|f| f.severity.blocks_merge() && !f.resolved)
                    .cloned()
                    .collect();

                let pending: Vec<_> = blocking_findings
                    .iter()
                    .map(|f| PendingFinding::from_review_finding(f, *iteration))
                    .collect();

                qa_loop.pending_findings = pending;
                review.merge_blocked = true;
            }
            QATransition::Escalated => {
                warn!(
                    task_id = %task.id,
                    reason = ?qa_loop.escalation_reason,
                    "QA loop escalated"
                );
                review.merge_blocked = true;
            }
        }

        let qa_metadata = qa_loop.to_metadata();
        let status = qa_loop.current_status();
        let merge_readiness = qa_loop.get_merge_readiness();

        let mut metadata = serde_json::json!({
            "review_summary": review,
            "qa_loop": qa_metadata,
            "qa_status": status,
            "merge_readiness": merge_readiness,
        });

        match transition {
            QATransition::Approved => {
                metadata["outcome"] = serde_json::json!("approved");
                Ok(PhaseResult::success(output).with_metadata(metadata))
            }
            QATransition::NeedsFix { .. } => {
                metadata["outcome"] = serde_json::json!("needs_fix");
                Ok(PhaseResult::failure(format!(
                    "Review blocked: {} findings need fixing",
                    gate_result.reasons.len()
                ))
                .with_metadata(metadata))
            }
            QATransition::Escalated => {
                metadata["outcome"] = serde_json::json!("escalated");
                Ok(PhaseResult::failure(format!(
                    "QA escalated: {}",
                    qa_loop
                        .escalation_reason
                        .as_deref()
                        .unwrap_or("Max retries exceeded")
                ))
                .with_metadata(metadata))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_finding(severity: ReviewSeverity) -> ReviewFinding {
        ReviewFinding {
            id: "test-1".to_string(),
            category: FindingCategory::Correctness,
            severity,
            title: "Test finding".to_string(),
            description: "Test description".to_string(),
            location: None,
            suggestion: Some("Fix it".to_string()),
            resolved: false,
        }
    }

    #[test]
    fn test_determine_outcome_blocks_on_critical() {
        let findings = vec![make_finding(ReviewSeverity::Critical)];
        let (approved, reason) = determine_review_outcome("no approval", &findings);
        assert!(!approved);
        assert!(reason.contains("Blocking"));
    }

    #[test]
    fn test_determine_outcome_blocks_on_high() {
        let findings = vec![make_finding(ReviewSeverity::High)];
        let (approved, reason) = determine_review_outcome("no approval", &findings);
        assert!(!approved);
        assert!(reason.contains("Blocking"));
    }

    #[test]
    fn test_determine_outcome_allows_non_blocking() {
        let findings = vec![
            make_finding(ReviewSeverity::Medium),
            make_finding(ReviewSeverity::Low),
        ];
        let (approved, reason) = determine_review_outcome("no approval", &findings);
        assert!(approved);
        assert!(reason.contains("non-blocking"));
    }

    #[test]
    fn test_determine_outcome_empty_findings_with_approval() {
        let findings = vec![];
        let (approved, _) = determine_review_outcome("REVIEW: APPROVED", &findings);
        assert!(approved);
    }

    #[test]
    fn test_determine_outcome_empty_findings_with_rejection() {
        let findings = vec![];
        let (approved, reason) = determine_review_outcome("REVIEW: REJECTED", &findings);
        assert!(!approved);
        assert!(reason.contains("rejected"));
    }

    #[test]
    fn test_determine_outcome_empty_findings_with_lgtm() {
        let findings = vec![];
        let (approved, _) = determine_review_outcome("Looks good, LGTM", &findings);
        assert!(approved);
    }

    #[test]
    fn test_determine_outcome_empty_findings_no_signal() {
        let findings = vec![];
        let (approved, reason) = determine_review_outcome("some random text", &findings);
        assert!(!approved);
        assert!(reason.contains("No findings"));
    }

    #[test]
    fn test_generate_rationale_no_findings() {
        let rationale = generate_review_rationale(&[]);
        assert!(rationale.contains("No issues"));
        assert!(rationale.contains("ready"));
    }

    #[test]
    fn test_generate_rationale_blocking_findings() {
        let findings = vec![make_finding(ReviewSeverity::Critical)];
        let rationale = generate_review_rationale(&findings);
        assert!(rationale.contains("blocking"));
        assert!(rationale.contains("must be resolved"));
    }

    #[test]
    fn test_generate_rationale_non_blocking_findings() {
        let findings = vec![make_finding(ReviewSeverity::Medium)];
        let rationale = generate_review_rationale(&findings);
        assert!(rationale.contains("non-blocking"));
        assert!(rationale.contains("ready"));
    }

    #[test]
    fn test_parse_severity() {
        assert!(parse_severity("critical").is_some());
        assert!(parse_severity("CRITICAL").is_some());
        assert!(parse_severity("crit").is_some());
        assert!(parse_severity("high").is_some());
        assert!(parse_severity("medium").is_some());
        assert!(parse_severity("warning").is_some());
        assert!(parse_severity("low").is_some());
        assert!(parse_severity("info").is_some());
        assert!(parse_severity("unknown").is_none());
    }

    #[test]
    fn test_parse_category() {
        assert!(parse_category("security").is_some());
        assert!(parse_category("SEC").is_some());
        assert!(parse_category("correctness").is_some());
        assert!(parse_category("bug").is_some());
        assert!(parse_category("performance").is_some());
        assert!(parse_category("coverage").is_some());
        assert!(parse_category("breaking").is_some());
        assert!(parse_category("docs").is_some());
        assert!(parse_category("unknown").is_none());
    }

    #[test]
    fn test_find_matching_bracket() {
        let s = "[{[]}]";
        assert!(find_matching_bracket(s, 0).is_some());

        let s2 = "no brackets";
        assert!(find_matching_bracket(s2, 0).is_none());
    }
}
