//! Individual reaction handlers for each event type.

use super::engine::ReactionEngine;
use super::types::*;

impl ReactionEngine {
    /// Handle CI failure events
    pub(super) async fn handle_ci_failure(&self, event: &ReactionEvent) -> HandlerDecision {
        let config = &self.config.ci_failure;
        if !config.enabled {
            return HandlerDecision::new(
                ReactionType::NoAction,
                "CI failure reactions are disabled".to_string(),
            );
        }

        let retry_count = self.get_retry_count(event).await;

        if retry_count >= config.max_retries {
            if config.notify_on_failure {
                return HandlerDecision::new(
                    ReactionType::Notify,
                    format!(
                        "Max retries ({}) exceeded for CI failure",
                        config.max_retries
                    ),
                )
                .with_metadata("retry_count".to_string(), retry_count.to_string());
            } else {
                return HandlerDecision::new(
                    ReactionType::Cancel,
                    "Max retries exceeded, task cancelled".to_string(),
                );
            }
        }

        if config.auto_fix {
            self.increment_retry_count(event).await;
            {
                let mut stats = self.stats.write().await;
                stats.auto_fix_attempts += 1;
            }
            HandlerDecision::new(
                ReactionType::AutoFix,
                format!(
                    "Attempting auto-fix (attempt {}/{})",
                    retry_count + 1,
                    config.max_retries
                ),
            )
            .with_metadata("retry_count".to_string(), (retry_count + 1).to_string())
        } else {
            HandlerDecision::new(
                ReactionType::Notify,
                "Auto-fix disabled, notifying human".to_string(),
            )
        }
    }

    /// Handle review comment events
    pub(super) async fn handle_review_comment(&self, event: &ReactionEvent) -> HandlerDecision {
        let config = &self.config.review_comment;
        if !config.enabled {
            return HandlerDecision::new(
                ReactionType::NoAction,
                "Review comment reactions are disabled".to_string(),
            );
        }

        if let ReactionEvent::ReviewComment {
            body,
            changes_requested,
            ..
        } = event
        {
            if *changes_requested {
                let body_lower = body.to_lowercase();
                let is_trivial = config
                    .trivial_keywords
                    .iter()
                    .any(|kw| body_lower.contains(&kw.to_lowercase()));
                let is_complex = config
                    .complex_keywords
                    .iter()
                    .any(|kw| body_lower.contains(&kw.to_lowercase()));

                if is_complex && config.notify_on_complex {
                    return HandlerDecision::new(
                        ReactionType::Escalate,
                        "Complex changes requested, escalating".to_string(),
                    );
                }
                if is_trivial && config.auto_fix_trivial {
                    return HandlerDecision::new(
                        ReactionType::AutoFix,
                        "Trivial changes requested, attempting auto-fix".to_string(),
                    );
                }
                if config.notify_on_complex {
                    return HandlerDecision::new(
                        ReactionType::Notify,
                        "Changes requested, notifying human".to_string(),
                    );
                }
            }

            let body_lower = body.to_lowercase();
            let is_trivial = config
                .trivial_keywords
                .iter()
                .any(|kw| body_lower.contains(&kw.to_lowercase()));
            if is_trivial && config.auto_fix_trivial {
                return HandlerDecision::new(
                    ReactionType::AutoFix,
                    "Trivial comment detected, attempting auto-fix".to_string(),
                );
            }
            HandlerDecision::new(
                ReactionType::Notify,
                "Review comment received, notifying human".to_string(),
            )
        } else {
            HandlerDecision::new(
                ReactionType::NoAction,
                "Invalid event type for review comment handler".to_string(),
            )
        }
    }

    /// Handle merge conflict events
    pub(super) async fn handle_merge_conflict(&self, event: &ReactionEvent) -> HandlerDecision {
        let config = &self.config.merge_conflict;
        if !config.enabled {
            return HandlerDecision::new(
                ReactionType::NoAction,
                "Merge conflict reactions are disabled".to_string(),
            );
        }

        if let ReactionEvent::MergeConflict {
            conflicted_files, ..
        } = event
        {
            if config.auto_resolve {
                let retry_count = self.get_retry_count(event).await;
                if retry_count < config.max_resolution_attempts {
                    self.increment_retry_count(event).await;
                    return HandlerDecision::new(
                        ReactionType::AutoFix,
                        format!(
                            "Attempting auto-resolution (attempt {}/{})",
                            retry_count + 1,
                            config.max_resolution_attempts
                        ),
                    )
                    .with_metadata("conflicted_files".to_string(), conflicted_files.join(","));
                }
            }

            if config.notify_always {
                return HandlerDecision::new(
                    ReactionType::Notify,
                    format!(
                        "Merge conflict in {} file(s), notifying human",
                        conflicted_files.len()
                    ),
                )
                .with_metadata("conflicted_files".to_string(), conflicted_files.join(","));
            }

            HandlerDecision::new(
                ReactionType::Checkpoint,
                "Merge conflict detected, creating checkpoint".to_string(),
            )
            .with_metadata("conflicted_files".to_string(), conflicted_files.join(","))
        } else {
            HandlerDecision::new(
                ReactionType::NoAction,
                "Invalid event type for merge conflict handler".to_string(),
            )
        }
    }

    /// Handle agent idle events
    pub(super) async fn handle_agent_idle(&self, event: &ReactionEvent) -> HandlerDecision {
        let config = &self.config.agent_idle;
        if !config.enabled {
            return HandlerDecision::new(
                ReactionType::NoAction,
                "Agent idle reactions are disabled".to_string(),
            );
        }

        if let ReactionEvent::AgentIdle {
            idle_duration_secs, ..
        } = event
        {
            if *idle_duration_secs >= config.max_idle_secs {
                return HandlerDecision::new(
                    ReactionType::Cancel,
                    format!(
                        "Agent idle for {}s exceeds maximum ({}s), cancelling task",
                        idle_duration_secs, config.max_idle_secs
                    ),
                );
            }

            if *idle_duration_secs >= config.idle_timeout_secs && config.notify_on_stuck {
                if config.checkpoint_before_notify {
                    return HandlerDecision::new(
                        ReactionType::Checkpoint,
                        "Agent appears stuck, checkpointing before notification".to_string(),
                    )
                    .with_metadata(
                        "idle_duration_secs".to_string(),
                        idle_duration_secs.to_string(),
                    )
                    .with_metadata("should_notify".to_string(), "true".to_string());
                }
                return HandlerDecision::new(
                    ReactionType::Notify,
                    format!(
                        "Agent idle for {}s (threshold: {}s)",
                        idle_duration_secs, config.idle_timeout_secs
                    ),
                )
                .with_metadata(
                    "idle_duration_secs".to_string(),
                    idle_duration_secs.to_string(),
                );
            }

            HandlerDecision::new(
                ReactionType::NoAction,
                format!(
                    "Agent idle for {}s, below threshold ({}s)",
                    idle_duration_secs, config.idle_timeout_secs
                ),
            )
        } else {
            HandlerDecision::new(
                ReactionType::NoAction,
                "Invalid event type for agent idle handler".to_string(),
            )
        }
    }

    /// Handle task failed events
    pub(super) async fn handle_task_failed(&self, event: &ReactionEvent) -> HandlerDecision {
        if let ReactionEvent::TaskFailed {
            error,
            failed_phase,
            retry_count,
            ..
        } = event
        {
            let mut decision = HandlerDecision::new(
                ReactionType::Checkpoint,
                format!("Task failed at {:?}: {}", failed_phase, error),
            );
            decision = decision
                .with_metadata("error".to_string(), error.clone())
                .with_metadata("retry_count".to_string(), retry_count.to_string());
            if let Some(phase) = failed_phase {
                decision =
                    decision.with_metadata("failed_phase".to_string(), format!("{:?}", phase));
            }
            decision
        } else {
            HandlerDecision::new(
                ReactionType::NoAction,
                "Invalid event type for task failed handler".to_string(),
            )
        }
    }
}
