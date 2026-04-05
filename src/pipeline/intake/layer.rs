//! Task intake layer implementation
//!
//! Normalizes various trigger sources into consistent task records.
//! This is the ONLY way to create tasks in the system.

use anyhow::Result;
use chrono::Utc;
use tracing::{debug, info};

use super::super::phases::{Phase, Priority, Task, TaskStatus};
use super::types::{TaskIntakeInput, TaskSource};

/// Task intake layer that normalizes all trigger sources into consistent task records.
/// This is the ONLY way to create tasks in the system.
pub struct TaskIntake {
    /// ID counter prefix for generating task IDs
    id_prefix: String,
    /// Counter for generating sequential IDs
    counter: std::sync::atomic::AtomicU64,
}

impl TaskIntake {
    /// Create a new task intake layer
    pub fn new(id_prefix: impl Into<String>) -> Self {
        Self {
            id_prefix: id_prefix.into(),
            counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Generate a unique task ID
    fn generate_task_id(&self) -> String {
        let num = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let timestamp = Utc::now().format("%Y%m%d");
        format!("{}-{}-{:04}", self.id_prefix, timestamp, num)
    }

    /// Normalize an intake input into a task
    pub fn normalize_to_task(&self, input: TaskIntakeInput) -> Result<Task> {
        info!("Normalizing task from source: {}", input.source.label());

        // Generate task ID
        let task_id = self.generate_task_id();

        // Determine priority
        let priority = input.priority.unwrap_or_else(|| {
            self.infer_priority(&input.source, &input.title, &input.instruction)
        });

        // Determine initial phase
        let phase = input.initial_phase.unwrap_or(Phase::Research);

        // Build metadata
        let mut metadata = serde_json::to_value(&input.metadata).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(ref mut map) = metadata {
            // Add source information
            map.insert("source".to_string(), serde_json::to_value(&input.source)?);
            map.insert("tags".to_string(), serde_json::to_value(&input.tags)?);
            map.insert(
                "depends_on".to_string(),
                serde_json::to_value(&input.depends_on)?,
            );
            map.insert(
                "intake_timestamp".to_string(),
                serde_json::to_value(Utc::now().to_rfc3339())?,
            );
        }

        // Create the task
        let task = Task::new(&task_id, &input.title, &input.instruction)
            .with_phase(phase)
            .with_status(TaskStatus::Backlog)
            .with_priority(priority);

        // Add metadata using a mutable reference
        let mut task = task;
        task.metadata = metadata;

        debug!("Created task {} from source {:?}", task.id, input.source);

        Ok(task)
    }

    /// Infer priority from source and content
    fn infer_priority(&self, source: &TaskSource, title: &str, instruction: &str) -> Priority {
        // CI failures are always critical
        if matches!(source, TaskSource::CIFailure { .. }) {
            return Priority::Critical;
        }

        // PR comments are typically high priority
        if matches!(source, TaskSource::GitHubPRComment { .. }) {
            return Priority::High;
        }

        // Check for urgency keywords in title/instruction
        let content = format!("{} {}", title, instruction).to_lowercase();
        let urgent_keywords = [
            "urgent",
            "critical",
            "asap",
            "emergency",
            "blocker",
            "broken",
            "crash",
            "security",
            "vulnerability",
        ];

        for keyword in &urgent_keywords {
            if content.contains(keyword) {
                return Priority::Critical;
            }
        }

        // Check for high priority keywords
        let high_keywords = ["important", "priority", "deadline", "needed"];
        for keyword in &high_keywords {
            if content.contains(keyword) {
                return Priority::High;
            }
        }

        // Default priority
        Priority::Normal
    }

    /// Validate that a task can be created (check dependencies, etc.)
    pub fn validate_intake(&self, input: &TaskIntakeInput) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for external validation requirements
        if input.source.requires_external_validation() {
            warnings.push(format!(
                "Source {} may require external validation (e.g., issue still open)",
                input.source.label()
            ));
        }

        // Check for empty instruction
        if input.instruction.trim().is_empty() {
            anyhow::bail!("Task instruction cannot be empty");
        }

        // Check for very long instructions (might need decomposition)
        if input.instruction.len() > 10000 {
            warnings.push(
                "Instruction is very long (>10k chars), consider decomposing into smaller tasks"
                    .to_string(),
            );
        }

        // Check for dependencies
        if !input.depends_on.is_empty() {
            warnings.push(format!(
                "Task has {} dependencies that must be resolved before execution",
                input.depends_on.len()
            ));
        }

        Ok(warnings)
    }
}

impl Default for TaskIntake {
    fn default() -> Self {
        Self::new("TASK")
    }
}
