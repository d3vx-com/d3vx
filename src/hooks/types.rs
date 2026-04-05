//! Hook Types
//!
//! Defines hook event types, hook definitions, and execution results
//! for the extensible hooks system.

use std::collections::HashMap;
use std::path::PathBuf;

/// Events that can trigger hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookEvent {
    /// Before a tool is executed. Can block execution.
    PreToolUse { tool_name: String },
    /// After a tool executes successfully.
    PostToolUse { tool_name: String },
    /// After a tool execution fails.
    PostToolUseFailure { tool_name: String },
    /// When the agent stops (completes or errors).
    Stop { reason: String },
    /// When a session starts.
    SessionStart,
    /// When user submits a prompt.
    UserPromptSubmit,
    /// Before context compaction.
    PreCompact { trigger: CompactTrigger },
}

impl HookEvent {
    /// Returns a discriminant name for matching hooks to events.
    /// Used to compare event families (e.g., all PreToolUse events match).
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::PreToolUse { .. } => "PreToolUse",
            Self::PostToolUse { .. } => "PostToolUse",
            Self::PostToolUseFailure { .. } => "PostToolUseFailure",
            Self::Stop { .. } => "Stop",
            Self::SessionStart => "SessionStart",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreCompact { .. } => "PreCompact",
        }
    }

    /// Returns the tool name if this is a tool-related event.
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::PreToolUse { tool_name }
            | Self::PostToolUse { tool_name }
            | Self::PostToolUseFailure { tool_name } => Some(tool_name),
            _ => None,
        }
    }
}

/// What triggered a compaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactTrigger {
    Manual,
    Auto,
}

/// How a hook is implemented.
#[derive(Debug, Clone)]
pub enum HookKind {
    /// Run a shell command. Stdout is captured and parsed as JSON.
    Command { command: String },
    /// Evaluate a condition with the LLM (returns approve/block).
    Prompt { template: String },
}

/// A hook definition.
#[derive(Debug, Clone)]
pub struct HookDefinition {
    /// Unique name for this hook.
    pub name: String,
    /// The event that triggers this hook.
    pub event: HookEvent,
    /// How the hook is implemented.
    pub kind: HookKind,
    /// Whether this hook is enabled.
    pub enabled: bool,
}

impl HookDefinition {
    /// Check if this hook matches the given event.
    /// Matches if the variant is the same and the tool name matches (if applicable).
    pub fn matches(&self, event: &HookEvent) -> bool {
        if !self.enabled {
            return false;
        }
        if self.event.variant_name() != event.variant_name() {
            return false;
        }
        // For tool events, match if the hook has no specific tool or tools match.
        match (&self.event, event) {
            (
                HookEvent::PreToolUse {
                    tool_name: hook_tool,
                },
                HookEvent::PreToolUse {
                    tool_name: event_tool,
                },
            )
            | (
                HookEvent::PostToolUse {
                    tool_name: hook_tool,
                },
                HookEvent::PostToolUse {
                    tool_name: event_tool,
                },
            )
            | (
                HookEvent::PostToolUseFailure {
                    tool_name: hook_tool,
                },
                HookEvent::PostToolUseFailure {
                    tool_name: event_tool,
                },
            ) => hook_tool == event_tool || hook_tool == "*",
            _ => true,
        }
    }
}

/// Whether to allow or block the operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Allow the operation to proceed.
    Approve,
    /// Block the operation.
    Block,
    /// No opinion, continue.
    Pass,
}

impl Default for HookDecision {
    fn default() -> Self {
        Self::Pass
    }
}

/// Result of hook execution.
#[derive(Debug, Clone)]
pub struct HookOutput {
    /// Whether to continue or block the operation.
    pub decision: HookDecision,
    /// Message to show to the user.
    pub message: Option<String>,
    /// Additional context to inject back to the model.
    pub additional_context: Option<String>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            decision: HookDecision::Pass,
            message: None,
            additional_context: None,
        }
    }
}

impl HookOutput {
    /// Create an approve output with optional context.
    pub fn approve() -> Self {
        Self {
            decision: HookDecision::Approve,
            ..Self::default()
        }
    }

    /// Create a block output with a reason.
    pub fn block(message: impl Into<String>) -> Self {
        Self {
            decision: HookDecision::Block,
            message: Some(message.into()),
            additional_context: None,
        }
    }

    /// Merge another output into this one.
    /// Block takes priority. Contexts are accumulated.
    pub fn merge(&mut self, other: HookOutput) {
        // Block always wins.
        if other.decision == HookDecision::Block && self.decision != HookDecision::Block {
            self.decision = HookDecision::Block;
            if other.message.is_some() && self.message.is_none() {
                self.message = other.message;
            }
        } else if self.decision == HookDecision::Pass && other.decision != HookDecision::Pass {
            self.decision = other.decision;
            if self.message.is_none() {
                self.message = other.message;
            }
        }
        // Accumulate additional context.
        match (&self.additional_context, other.additional_context) {
            (Some(existing), Some(new)) => {
                self.additional_context = Some(format!("{}\n{}", existing, new));
            }
            (None, Some(new)) => {
                self.additional_context = Some(new);
            }
            _ => {}
        }
    }
}

/// Context provided to hook execution.
#[derive(Debug, Clone)]
pub struct HookExecutionContext {
    /// The event that triggered this hook.
    pub event: HookEvent,
    /// Tool input (for tool-related events).
    pub tool_input: Option<serde_json::Value>,
    /// Tool output (for post-execution events).
    pub tool_output: Option<String>,
    /// Current working directory.
    pub working_dir: PathBuf,
    /// Session ID.
    pub session_id: Option<String>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl HookExecutionContext {
    /// Create a minimal context for an event with a working directory.
    pub fn new(event: HookEvent, working_dir: PathBuf) -> Self {
        Self {
            event,
            tool_input: None,
            tool_output: None,
            working_dir,
            session_id: None,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_event_variant_name() {
        let event = HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        };
        assert_eq!(event.variant_name(), "PreToolUse");

        let event = HookEvent::SessionStart;
        assert_eq!(event.variant_name(), "SessionStart");
    }

    #[test]
    fn hook_event_tool_name() {
        let event = HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        };
        assert_eq!(event.tool_name(), Some("Bash"));

        let event = HookEvent::SessionStart;
        assert_eq!(event.tool_name(), None);
    }

    #[test]
    fn hook_definition_matches_same_variant() {
        let def = HookDefinition {
            name: "test".to_string(),
            event: HookEvent::PreToolUse {
                tool_name: "Bash".to_string(),
            },
            kind: HookKind::Command {
                command: "echo ok".to_string(),
            },
            enabled: true,
        };

        let event = HookEvent::PreToolUse {
            tool_name: "Bash".to_string(),
        };
        assert!(def.matches(&event));
    }

    #[test]
    fn hook_definition_does_not_match_different_variant() {
        let def = HookDefinition {
            name: "test".to_string(),
            event: HookEvent::PreToolUse {
                tool_name: "Bash".to_string(),
            },
            kind: HookKind::Command {
                command: "echo ok".to_string(),
            },
            enabled: true,
        };

        let event = HookEvent::PostToolUse {
            tool_name: "Bash".to_string(),
        };
        assert!(!def.matches(&event));
    }

    #[test]
    fn hook_definition_disabled_never_matches() {
        let def = HookDefinition {
            name: "test".to_string(),
            event: HookEvent::SessionStart,
            kind: HookKind::Command {
                command: "echo ok".to_string(),
            },
            enabled: false,
        };

        assert!(!def.matches(&HookEvent::SessionStart));
    }

    #[test]
    fn hook_definition_wildcard_matches_any_tool() {
        let def = HookDefinition {
            name: "test".to_string(),
            event: HookEvent::PreToolUse {
                tool_name: "*".to_string(),
            },
            kind: HookKind::Command {
                command: "echo ok".to_string(),
            },
            enabled: true,
        };

        let event = HookEvent::PreToolUse {
            tool_name: "Read".to_string(),
        };
        assert!(def.matches(&event));
    }

    #[test]
    fn hook_output_merge_block_wins() {
        let mut output = HookOutput::approve();
        output.merge(HookOutput::block("blocked"));
        assert_eq!(output.decision, HookDecision::Block);
        assert_eq!(output.message.as_deref(), Some("blocked"));
    }

    #[test]
    fn hook_output_merge_approve_overrides_pass() {
        let mut output = HookOutput::default();
        output.merge(HookOutput::approve());
        assert_eq!(output.decision, HookDecision::Approve);
    }

    #[test]
    fn hook_output_merge_accumulates_context() {
        let mut output = HookOutput {
            decision: HookDecision::Approve,
            message: None,
            additional_context: Some("ctx1".to_string()),
        };
        output.merge(HookOutput {
            decision: HookDecision::Pass,
            message: None,
            additional_context: Some("ctx2".to_string()),
        });
        assert_eq!(output.additional_context.as_deref(), Some("ctx1\nctx2"));
    }

    #[test]
    fn execution_context_new() {
        let ctx = HookExecutionContext::new(HookEvent::SessionStart, PathBuf::from("/tmp"));
        assert!(ctx.tool_input.is_none());
        assert!(ctx.session_id.is_none());
        assert!(ctx.metadata.is_empty());
    }
}
