//! Inter-Agent Coordination Module
//!
//! Lightweight coordination for multi-agent batches:
//! - Structured messages between parent and children
//! - Blocker reporting when children are stuck
//! - Handoff notes for dependency communication
//! - Progress updates for parent visibility
//! - Structured synthesis inputs
//!
//! ## Design Principles
//!
//! 1. **Lightweight** - Simple structs, no heavy transport
//! 2. **Structured** - Typed messages, not raw text
//! 3. **Observable** - State visible in metadata and UI
//! 4. **Bounded** - No recursive swarm explosion

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Category of coordination message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationType {
    /// Child is blocked waiting on a dependency
    BlockerReport,
    /// Child has output needed by another child
    HandoffNote,
    /// Child is reporting progress
    ProgressUpdate,
    /// Child has findings that should feed synthesis
    SynthesisInput,
    /// Child needs input from parent
    ParentRequest,
    /// Child is reporting completion with artifacts
    CompletionReport,
}

impl std::fmt::Display for CoordinationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockerReport => write!(f, "blocker_report"),
            Self::HandoffNote => write!(f, "handoff_note"),
            Self::ProgressUpdate => write!(f, "progress_update"),
            Self::SynthesisInput => write!(f, "synthesis_input"),
            Self::ParentRequest => write!(f, "parent_request"),
            Self::CompletionReport => write!(f, "completion_report"),
        }
    }
}

/// Priority level for coordination messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordinationPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl CoordinationPriority {
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Critical | Self::High)
    }
}

/// A structured coordination message between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationMessage {
    pub id: String,
    pub msg_type: CoordinationType,
    pub from_key: String,
    pub to_key: Option<String>,
    pub priority: CoordinationPriority,
    pub subject: String,
    pub body: String,
    pub timestamp: String,
    pub resolved: bool,
    pub metadata: CoordinationMetadata,
}

impl CoordinationMessage {
    pub fn new(
        msg_type: CoordinationType,
        from_key: String,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("coord-{}-{}", msg_type, uuid::Uuid::new_v4().as_simple()),
            msg_type,
            from_key,
            to_key: None,
            priority: CoordinationPriority::Normal,
            subject: subject.into(),
            body: body.into(),
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata::default(),
        }
    }

    pub fn to(to_key: impl Into<String>) -> CoordinationMessageBuilder {
        CoordinationMessageBuilder {
            msg_type: CoordinationType::ProgressUpdate,
            from_key: String::new(),
            to_key: Some(to_key.into()),
            priority: CoordinationPriority::Normal,
            subject: String::new(),
            body: String::new(),
            metadata: CoordinationMetadata::default(),
        }
    }

    pub fn blocker_report(from_key: String, blocked_on: String, reason: String) -> Self {
        Self {
            id: format!("blocker-{}-{}", from_key, uuid::Uuid::new_v4().as_simple()),
            msg_type: CoordinationType::BlockerReport,
            from_key,
            to_key: None,
            priority: CoordinationPriority::High,
            subject: format!("Blocked on: {}", blocked_on),
            body: reason,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata {
                blocker_kind: Some(BlockerKind::DependencyWaiting),
                ..Default::default()
            },
        }
    }

    pub fn handoff_note(
        from_key: String,
        to_key: String,
        artifact_name: String,
        description: String,
    ) -> Self {
        Self {
            id: format!(
                "handoff-{}-{}-{}",
                from_key,
                to_key,
                uuid::Uuid::new_v4().as_simple()
            ),
            msg_type: CoordinationType::HandoffNote,
            from_key,
            to_key: Some(to_key),
            priority: CoordinationPriority::Normal,
            subject: format!("Handoff: {}", artifact_name),
            body: description,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata {
                artifact_name: Some(artifact_name),
                ..Default::default()
            },
        }
    }

    pub fn progress_update(from_key: String, progress: u8, message: String) -> Self {
        Self {
            id: format!("progress-{}-{}", from_key, uuid::Uuid::new_v4().as_simple()),
            msg_type: CoordinationType::ProgressUpdate,
            from_key,
            to_key: None,
            priority: CoordinationPriority::Low,
            subject: format!("Progress: {}%", progress),
            body: message,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata {
                progress_percent: Some(progress),
                ..Default::default()
            },
        }
    }

    pub fn synthesis_input(from_key: String, finding: String, category: String) -> Self {
        Self {
            id: format!("input-{}-{}", from_key, uuid::Uuid::new_v4().as_simple()),
            msg_type: CoordinationType::SynthesisInput,
            from_key,
            to_key: None,
            priority: CoordinationPriority::Normal,
            subject: format!("Finding: {}", category),
            body: finding,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata {
                finding_category: Some(category),
                ..Default::default()
            },
        }
    }

    pub fn parent_request(from_key: String, request: String, context: String) -> Self {
        Self {
            id: format!("request-{}-{}", from_key, uuid::Uuid::new_v4().as_simple()),
            msg_type: CoordinationType::ParentRequest,
            from_key,
            to_key: None,
            priority: CoordinationPriority::High,
            subject: "Request from child".to_string(),
            body: format!("{}\n\nContext:\n{}", request, context),
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata::default(),
        }
    }

    pub fn completion_report(
        from_key: String,
        summary: String,
        files_changed: Vec<String>,
        decisions: Vec<String>,
        issues: Vec<String>,
    ) -> Self {
        let id = format!("complete-{}-{}", from_key, uuid::Uuid::new_v4().as_simple());
        let subject = format!("Completed: {}", from_key);
        Self {
            id,
            msg_type: CoordinationType::CompletionReport,
            from_key,
            to_key: None,
            priority: CoordinationPriority::Normal,
            subject,
            body: summary,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: CoordinationMetadata {
                files_changed,
                decisions,
                issues,
                ..Default::default()
            },
        }
    }
}

/// Builder pattern for coordination messages
#[derive(Debug, Clone)]
pub struct CoordinationMessageBuilder {
    msg_type: CoordinationType,
    from_key: String,
    to_key: Option<String>,
    priority: CoordinationPriority,
    subject: String,
    body: String,
    metadata: CoordinationMetadata,
}

impl CoordinationMessageBuilder {
    pub fn msg_type(mut self, msg_type: CoordinationType) -> Self {
        self.msg_type = msg_type;
        self
    }

    pub fn from_key(mut self, from_key: impl Into<String>) -> Self {
        self.from_key = from_key.into();
        self
    }

    pub fn to(mut self, to_key: impl Into<String>) -> Self {
        self.to_key = Some(to_key.into());
        self
    }

    pub fn priority(mut self, priority: CoordinationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn metadata(mut self, metadata: CoordinationMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn build(self) -> CoordinationMessage {
        CoordinationMessage {
            id: format!(
                "coord-{}-{}",
                self.msg_type,
                uuid::Uuid::new_v4().as_simple()
            ),
            msg_type: self.msg_type,
            from_key: self.from_key,
            to_key: self.to_key,
            priority: self.priority,
            subject: self.subject,
            body: self.body,
            timestamp: Utc::now().to_rfc3339(),
            resolved: false,
            metadata: self.metadata,
        }
    }
}

/// Additional metadata for coordination messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoordinationMetadata {
    pub blocker_kind: Option<BlockerKind>,
    pub artifact_name: Option<String>,
    pub progress_percent: Option<u8>,
    pub finding_category: Option<String>,
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub issues: Vec<String>,
}

/// Kind of blocker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    DependencyWaiting,
    InputRequired,
    ResourceUnavailable,
    ExternalApiFailure,
    HumanInputNeeded,
}

/// Structured synthesis input from children
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisInput {
    pub child_key: String,
    pub category: String,
    pub finding: String,
    pub severity: SynthesisSeverity,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SynthesisSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Batch coordination state for tracking messages and progress
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchCoordination {
    #[serde(default)]
    pub messages: Vec<CoordinationMessage>,
    #[serde(default)]
    pub synthesis_inputs: Vec<SynthesisInput>,
    #[serde(default)]
    pub unresolved_blockers: Vec<UnresolvedBlocker>,
    pub last_progress_update: Option<String>,
}

impl BatchCoordination {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_message(&mut self, msg: CoordinationMessage) {
        self.messages.push(msg);
    }

    pub fn add_synthesis_input(&mut self, input: SynthesisInput) {
        self.synthesis_inputs.push(input);
    }

    pub fn add_blocker(&mut self, blocker: UnresolvedBlocker) {
        self.unresolved_blockers.push(blocker);
    }

    pub fn resolve_blocker(&mut self, child_key: &str, blocker_id: &str) {
        for blocker in &mut self.unresolved_blockers {
            if blocker.child_key == child_key && blocker.id == blocker_id {
                blocker.resolved_at = Some(Utc::now().to_rfc3339());
            }
        }
        for msg in &mut self.messages {
            if msg.from_key == child_key && msg.id == blocker_id {
                msg.resolved = true;
            }
        }
    }

    pub fn get_blockers_for(&self, child_key: &str) -> Vec<&UnresolvedBlocker> {
        self.unresolved_blockers
            .iter()
            .filter(|b| b.child_key == child_key && b.resolved_at.is_none())
            .collect()
    }

    pub fn get_handoffs_for(&self, child_key: &str) -> Vec<&CoordinationMessage> {
        self.messages
            .iter()
            .filter(|m| m.to_key.as_deref() == Some(child_key) && !m.resolved)
            .collect()
    }

    pub fn get_handoffs_from(&self, child_key: &str) -> Vec<&CoordinationMessage> {
        self.messages
            .iter()
            .filter(|m| m.from_key == child_key && !m.resolved)
            .collect()
    }

    pub fn has_blockers(&self) -> bool {
        self.unresolved_blockers
            .iter()
            .any(|b| b.resolved_at.is_none())
    }

    pub fn blocking_count(&self) -> usize {
        self.unresolved_blockers
            .iter()
            .filter(|b| b.resolved_at.is_none() && b.priority.is_blocking())
            .count()
    }

    pub fn summary(&self) -> CoordinationSummary {
        CoordinationSummary {
            total_messages: self.messages.len(),
            unresolved_blockers: self
                .unresolved_blockers
                .iter()
                .filter(|b| b.resolved_at.is_none())
                .count(),
            synthesis_inputs: self.synthesis_inputs.len(),
            blocking_count: self.blocking_count(),
            last_progress: self.last_progress_update.clone(),
        }
    }
}

/// An unresolved blocker from a child
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedBlocker {
    pub id: String,
    pub child_key: String,
    pub kind: BlockerKind,
    pub description: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub priority: CoordinationPriority,
}

impl UnresolvedBlocker {
    pub fn new(child_key: String, kind: BlockerKind, description: String) -> Self {
        Self {
            id: format!("blocker-{}-{}", child_key, uuid::Uuid::new_v4().as_simple()),
            child_key,
            kind,
            description,
            created_at: Utc::now().to_rfc3339(),
            resolved_at: None,
            priority: CoordinationPriority::High,
        }
    }

    pub fn is_blocking(&self) -> bool {
        self.priority.is_blocking() && self.resolved_at.is_none()
    }
}

/// Summary of batch coordination state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationSummary {
    pub total_messages: usize,
    pub unresolved_blockers: usize,
    pub synthesis_inputs: usize,
    pub blocking_count: usize,
    pub last_progress: Option<String>,
}

impl CoordinationSummary {
    pub fn display(&self) -> String {
        let parts = vec![
            format!("{} messages", self.total_messages),
            format!("{} blockers", self.unresolved_blockers),
            format!("{} inputs", self.synthesis_inputs),
        ];
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocker_report() {
        let msg = CoordinationMessage::blocker_report(
            "child-1".to_string(),
            "child-0".to_string(),
            "Waiting for schema definition".to_string(),
        );

        assert_eq!(msg.msg_type, CoordinationType::BlockerReport);
        assert_eq!(msg.priority, CoordinationPriority::High);
        assert!(!msg.resolved);
    }

    #[test]
    fn test_handoff_note() {
        let msg = CoordinationMessage::handoff_note(
            "child-1".to_string(),
            "child-2".to_string(),
            "API Schema".to_string(),
            "Use this schema for endpoint definitions".to_string(),
        );

        assert_eq!(msg.msg_type, CoordinationType::HandoffNote);
        assert_eq!(msg.to_key, Some("child-2".to_string()));
        assert!(msg.metadata.artifact_name.is_some());
    }

    #[test]
    fn test_progress_update() {
        let msg = CoordinationMessage::progress_update(
            "child-1".to_string(),
            50,
            "Processing batch of files".to_string(),
        );

        assert_eq!(msg.msg_type, CoordinationType::ProgressUpdate);
        assert_eq!(msg.metadata.progress_percent, Some(50));
    }

    #[test]
    fn test_synthesis_input() {
        let msg = CoordinationMessage::synthesis_input(
            "child-reviewer".to_string(),
            "Missing error handling in auth module".to_string(),
            "Correctness".to_string(),
        );

        assert_eq!(msg.msg_type, CoordinationType::SynthesisInput);
        assert_eq!(
            msg.metadata.finding_category,
            Some("Correctness".to_string())
        );
    }

    #[test]
    fn test_batch_coordination_blockers() {
        let mut coord = BatchCoordination::new();

        let blocker = UnresolvedBlocker::new(
            "child-1".to_string(),
            BlockerKind::DependencyWaiting,
            "Waiting for child-0".to_string(),
        );
        let blocker_id = blocker.id.clone();
        coord.add_blocker(blocker);

        assert!(coord.has_blockers());
        assert_eq!(coord.unresolved_blockers.len(), 1);
        assert_eq!(coord.blocking_count(), 1);

        coord.resolve_blocker("child-1", &blocker_id);
        assert!(!coord.has_blockers());
    }

    #[test]
    fn test_handoffs_filtering() {
        let mut coord = BatchCoordination::new();

        coord.add_message(CoordinationMessage::handoff_note(
            "child-1".to_string(),
            "child-2".to_string(),
            "Shared Data".to_string(),
            "Use this".to_string(),
        ));

        coord.add_message(CoordinationMessage::handoff_note(
            "child-3".to_string(),
            "child-2".to_string(),
            "Shared Data 2".to_string(),
            "Use this too".to_string(),
        ));

        let for_child2 = coord.get_handoffs_for("child-2");
        assert_eq!(for_child2.len(), 2);

        let from_child1 = coord.get_handoffs_from("child-1");
        assert_eq!(from_child1.len(), 1);
    }

    #[test]
    fn test_completion_report() {
        let msg = CoordinationMessage::completion_report(
            "child-1".to_string(),
            "Implemented feature X".to_string(),
            vec!["src/feature.rs".to_string()],
            vec!["Used strategy pattern for extensibility".to_string()],
            vec![],
        );

        assert_eq!(msg.msg_type, CoordinationType::CompletionReport);
        assert_eq!(msg.metadata.files_changed.len(), 1);
        assert_eq!(msg.metadata.decisions.len(), 1);
        assert!(msg.metadata.issues.is_empty());
    }

    #[test]
    fn test_coordination_summary() {
        let mut coord = BatchCoordination::new();
        coord.add_message(CoordinationMessage::progress_update(
            "child-1".to_string(),
            100,
            "Done".to_string(),
        ));
        coord.add_message(CoordinationMessage::synthesis_input(
            "child-1".to_string(),
            "Finding".to_string(),
            "Category".to_string(),
        ));

        let summary = coord.summary();
        assert_eq!(summary.total_messages, 2);
        assert_eq!(summary.synthesis_inputs, 0); // synthesis_inputs Vec is separate from messages
        assert_eq!(summary.display(), "2 messages, 0 blockers, 0 inputs");
    }

    #[test]
    fn test_message_builder() {
        let msg = CoordinationMessage::to("child-2")
            .msg_type(CoordinationType::HandoffNote)
            .from_key("child-1")
            .subject("Test handoff")
            .body("Here is the data")
            .priority(CoordinationPriority::High)
            .build();

        assert_eq!(msg.to_key, Some("child-2".to_string()));
        assert_eq!(msg.from_key, "child-1");
        assert_eq!(msg.priority, CoordinationPriority::High);
    }

    #[test]
    fn test_blocker_kind() {
        let blocker = UnresolvedBlocker::new(
            "child-1".to_string(),
            BlockerKind::HumanInputNeeded,
            "Need decision".to_string(),
        );
        assert!(blocker.is_blocking());
    }

    #[test]
    fn test_priority_blocking() {
        assert!(CoordinationPriority::Critical.is_blocking());
        assert!(CoordinationPriority::High.is_blocking());
        assert!(!CoordinationPriority::Normal.is_blocking());
        assert!(!CoordinationPriority::Low.is_blocking());
    }
}
