//! Pipeline Engine Module
//!
//! Provides a 6-phase pipeline system for autonomous task execution:
//! Research -> Plan -> Draft -> Review -> Implement -> Docs
//!
//! # Overview
//!
//! The pipeline engine orchestrates task execution through a series of phases,
//! each with dedicated handlers. Tasks are managed via a priority-based queue
//! and can be executed concurrently across isolated git worktrees.
//!
//! # Architecture
//!
//! ```text
//! External Sources (Chat, GitHub, CI, Automation)
//!         |
//!         v
//! +-------------------+
//! |   TaskIntake      |  <-- Normalizes all inputs
//! +-------------------+
//!         |
//!         v
//! +-------------------+
//! | ExecutionClassifier| <-- Determines execution mode
//! +-------------------+
//!         |
//!         v
//! +-------------------+
//! |PipelineOrchestrator| <-- Central authority (TaskAuthority trait)
//! +-------------------+
//!    |      |      |
//!    v      v      v
//!  Queue WorkerPool  Checkpoint
//! ```
//!
//! # Creating Tasks
//!
//! Tasks should ONLY be created through the `PipelineOrchestrator`:
//!
//! ```rust,no_run
//! use d3vx::pipeline::{PipelineOrchestrator, OrchestratorConfig, TaskAuthority};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = OrchestratorConfig::default();
//!     let orchestrator = PipelineOrchestrator::new(config, None).await.unwrap();
//!
//!     // Create a task from chat input
//!     let task = orchestrator
//!         .create_task_from_chat("Fix bug", "Fix the login bug", None)
//!         .await
//!         .unwrap();
//!
//!     // Create a task from a GitHub issue
//!     let task = orchestrator
//!         .create_task_from_github_issue(42, "owner/repo", "author", "Title", "Body")
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use d3vx::pipeline::{PipelineEngine, Task, TaskStatus, Phase, PhaseContext, Priority};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create the pipeline engine
//!     let engine = PipelineEngine::new();
//!
//!     // Create a task
//!     let task = Task::new("TASK-001", "Implement feature X", "Detailed instructions...")
//!         .with_status(TaskStatus::Queued)
//!         .with_priority(Priority::High);
//!
//!     // Create execution context
//!     let context = PhaseContext::new(task.clone(), "/project/root", "/project/worktree");
//!
//!     // Run the task through the pipeline
//!     let result = engine.run(task, context).await;
//!
//!     match result {
//!         Ok(run_result) if run_result.success => {
//!             println!("Task completed successfully!");
//!         }
//!         Ok(run_result) => {
//!             println!("Task failed: {:?}", run_result.error);
//!         }
//!         Err(e) => {
//!             println!("Pipeline error: {}", e);
//!         }
//!     }
//! }
//! ```

pub mod activity;
pub mod approval;
pub mod checkpoint;
pub mod classifier;
pub mod commander;
pub mod conflicts;
pub mod cost_tracker;
pub mod dashboard;
pub mod decomposition;
pub mod docs_completeness;
pub mod engine;
pub mod github;
pub mod handlers;
pub mod heartbeat;
pub mod intake;
pub mod issue_coordinator;
pub mod issue_picker;
pub mod issue_runner;
pub mod issue_sync;
pub mod lifecycle;
pub mod merge_gate;
pub mod metrics;
pub mod orchestrator;
pub mod ownership;
pub mod permission;
pub mod phases;
pub mod pr_ci_loop;
pub mod pr_lifecycle;
pub mod prompts;
pub mod qa_integration;
pub mod qa_loop;
pub mod queue;
pub mod queue_manager;
pub mod reaction;
pub mod recovery;
pub mod recovery_manager;
pub mod resume;
pub mod review_gate;
pub mod review_response;
pub mod review_summary;
pub mod risk_assessment;
pub mod runtime;
pub mod scheduler;
pub mod scope;
pub mod sdd;
pub mod semantic_merge;
pub mod snapshot_policy;
pub mod spawner;
pub mod state_machine;
pub mod task_factory;
pub mod task_lifecycle;
pub mod timeout;
pub mod tool_permissions;
pub mod trust_parser;
pub mod validation_summary;
pub mod vex_manager;
pub mod worker_pool;
pub mod workspace_hooks;

// Re-export commonly used types at module level
pub use checkpoint::{Checkpoint, CheckpointManager};
pub use classifier::{
    ClassificationResult, ClassifierConfig, ComplexityMetrics, ExecutionClassifier, ExecutionMode,
    RiskIndicators,
};
pub use cost_tracker::{
    estimate_cost, ApiUsage, CostStats, CostTracker, CostTrackerConfig, CostTrackerError,
};
pub use engine::{PipelineConfig, PipelineEngine, PipelineRunResult};
pub use handlers::{
    create_handler, default_handlers, DocsHandler, ImplementHandler, PhaseError, PhaseHandler,
    PhaseResult, PlanHandler, ResearchHandler, ReviewHandler,
};
pub use intake::{TaskIntake, TaskIntakeInput, TaskSource};
pub use orchestrator::{OrchestratorConfig, PipelineOrchestrator, TaskAuthority};
pub use phases::{Phase, PhaseContext, Priority, Task, TaskStatus};
pub use queue::{QueueError, QueueStats, TaskDependency, TaskQueue};
pub use timeout::{DurationGuard, TimeoutConfig, TimeoutManager};
pub use vex_manager::{VexTaskHandle, VexTaskStatus};
pub use worker_pool::{
    Worker, WorkerId, WorkerLease, WorkerPool, WorkerPoolConfig, WorkerPoolError,
    WorkerPoolManager, WorkerPoolStats, WorkerStatus,
};

// Re-export heartbeat types
pub use heartbeat::{
    Heartbeat, HeartbeatConfig, HeartbeatError, HeartbeatManager, HeartbeatStats, LeaseId,
    LeaseState, StaleWorkerInfo, WorkerHealth, WorkerHeartbeatState,
};

// Re-export scope types
pub use scope::{
    find_nested_repos, find_repo_root, is_nested_repo, ScopeAwareWorkspace, ScopeError, ScopeMode,
    TaskScope,
};

// Re-export GitHub integration types
pub use github::{
    CIStatus, CheckOutput, CheckStatus, GitHubConfig, GitHubEvent, GitHubIntegration, GitHubIssue,
    GitHubPoller,
};

// Re-export recovery types
pub use recovery::{
    ReconcileAction, RecoveryConfig, RecoveryManager, RecoveryResult, TaskReconciler,
    TaskRecoveryInfo, WorkspaceRecoveryInfo,
};

// Re-export merge gate types
pub use merge_gate::{
    MergeBlockingReason, MergeGate, MergeGateConfig, MergeReadiness, MergeSignals, MergeSource,
    MergeWarning, SignalReadiness,
};

// Re-export review gate types
pub use review_gate::{BlockingReason, GateResult, ReviewGate};
pub use review_summary::{
    FindingCategory, FindingLocation, ReviewFinding, ReviewRequirements, ReviewSeverity,
    ReviewStatus, ReviewSummary, ReviewerType,
};

// Re-export validation summary types
pub use validation_summary::{Confidence, ValidationSummary, ValidationUiSummary};

// Re-export QA loop types
pub use qa_loop::{
    PendingFinding, QAConfig, QALoop, QALoopRecord, QAState, QAStatus, QATransition,
};

// Re-export docs completeness types
pub use docs_completeness::{
    DocType, DocsCompleteness, DocsCompletenessEvaluator, DocsSignal, DocsStatus,
};

// Re-export trust parser types
pub use trust_parser::UnifiedTrustData;

// Re-export decomposition types
pub use decomposition::{
    AggregationStrategy, ChildTaskDefinition, ChildTaskStatus, DecompositionError, DecompositionId,
    DecompositionManager, DecompositionPlan, DecompositionStatus, DependencyGraph,
    ExecutionStrategy, ParallelExecutor, ResultAggregator, TaskDecomposer,
};

// Re-export resume / session-snapshot types
pub use resume::{
    CompactResume, CompactedSnapshot, CompactionBoundary, EventCategory, EventData, EventLog,
    EventSeverity, ResumeError, ResumeManager, ResumeResult, SerializedMessage, SerializedToolCall,
    SessionEvent, SessionSnapshot, SnapshotInfo, ToolRecord,
};

// Re-export State Machine engine
pub use state_machine::{transition, StateError};

// Re-export task lifecycle types
pub use task_lifecycle::{
    DeliveryState, DeliveryStateMachine, DeliveryStateTransition, LifecycleError, StateTrigger,
};

// Re-export reaction engine types
pub use reaction::{
    ReactionAuditRecord, ReactionConfig, ReactionEngine, ReactionEvent, ReactionResult,
    ReactionStats, ReactionType,
};

// Re-export workspace hooks
pub use workspace_hooks::{
    load_workspace_config, resolve_project_root, HookCommand, SymlinkEntry, WorkspaceHookExecutor,
    WorkspaceHookResult, WorkspaceHooksConfig,
};

// Re-export PR lifecycle types
pub use pr_lifecycle::{PrError, PrLifecycleManager, PrMetadata, PrState};

// Re-export issue tracker sync types
pub use issue_sync::{ExternalIssue, IssueTracker, SyncError, SyncResult};

// Re-export plugin architecture types
pub use crate::plugin::{
    AgentBackendAdapter as AgentAdapter, AgentHandle, CheckResult, IssueInfo,
    NotifierAdapter as NotifierPlugin, PluginDescriptor, PluginError, PluginRegistry, PluginSlot,
    PrInfo, PrStatus, ReviewInfo, RuntimeAdapter as RuntimePlugin, ScmAdapter, TerminalAdapter,
    TerminalHandle, TrackerAdapter, WorkspaceAdapter as WorkspacePlugin,
};

// Re-export multi-runtime types
pub use runtime::{ProcessRuntime, TmuxRuntime};

// Re-export dashboard types
pub use dashboard::{Dashboard, DashboardConfig, DashboardError, DashboardEvent};

// Re-export model routing types (ComplexityTier from providers)
pub use crate::providers::ComplexityTier;

// Re-export approval flow types
pub use approval::{
    ApprovalConfig, ApprovalDecision, ApprovalError, ApprovalFlow, ApprovalState, ExecutionPlan,
    PlanStep, Planner, RiskLevel, SubmitResult,
};

// Re-export permission lifecycle types
pub use permission::{
    PermissionDecision, PermissionManager, PermissionReq, PermissionState, PermissionStateError,
    PermissionStats, RiskLevel as ToolRiskLevel,
};

// Re-export ownership management types
pub use ownership::{
    OwnerId, OwnerToken, OwnerType, OwnershipError, OwnershipEvent, OwnershipEventType,
    OwnershipManager, OwnershipResult, OwnershipState, OwnershipStats,
};

// Re-export commander validation types
pub use commander::{ValidationCommand, ValidationKind, ValidationResult, ValidationRunner};

// Re-export spawner types
pub use spawner::{
    parallel_launch, BranchSpec, IssueContext, IssueLauncher, LaunchConfig, LaunchError,
    PromptComposer, SpawnResult, SpawnStatus, TrackerKind,
};

// Re-export session lifecycle types
pub use lifecycle::{
    probe_agent_status, CompositeProbe, GitProbe, PhaseMetadata, PhaseTransition, SessionPhase,
    SessionSummary, SessionTracker, TransitionCause, TransitionError, TransitionProbe,
};

// Re-export SDD types
pub use sdd::{
    AgentProvider, IntegrationResult, PlanGate, Scope, SddConfig, SddDecomposer, SddError,
    SddExecutor, SddIntegrator, SddResult, SddSession, SddState, SddWorkflow, SpecExtractor,
    TaskSpec,
};

// Re-export risk assessment types
pub use risk_assessment::{
    DependencyRisk, FileRisk, RiskAssessment, RiskRecommendation, RollbackDifficulty,
};

// Re-export QA integration types
pub use qa_integration::{QAIntegration, QAIntegrationConfig, QAResult};

// Re-export semantic merge types
pub use semantic_merge::{
    IssueKind, MergeViolation, MergeViolationSeverity, SemanticMergeAnalyzer, SemanticsReport,
};

// Re-export issue picker types
pub use issue_picker::{IssuePicker, IssuePickerConfig, IssuePriorityScore, PickDecision};

// Re-export issue coordinator types
pub use issue_coordinator::{
    IssueCoordinationConfig, IssueCoordinationResult, IssueCoordinator, SingleIssueResult,
};

// Re-export issue runner types
pub use issue_runner::{IssueRunResult, IssueRunner, IssueRunnerState};

// Re-export PR CI loop types
pub use pr_ci_loop::{
    CheckStatusDetail, CiFixConfig, CiFixLoop, CiFixResult, PrComment, PullRequestCIStatus,
};

// Re-export review response types
pub use review_response::{
    ActionableFeedback, ReviewCommentsReport, ReviewResponseLoop, ReviewResponseResult,
};

// Re-export context-aware permission types
pub use tool_permissions::{
    AutoApproveReason, ContextPermissionCache, ContextPermissionConfig, ContextPermissionResult,
};

#[cfg(test)]
mod tests;
