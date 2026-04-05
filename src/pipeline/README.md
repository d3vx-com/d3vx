# Pipeline Engine Documentation

## Overview

The d3vx Pipeline Engine is a production-grade task orchestration system that executes software engineering tasks through a structured 7-phase workflow with SDD (Subagent-Driven Development), QA integration, risk assessment, and semantic merge analysis:

```
Research → Ideation → Plan → Draft → Review → Implement → Docs
```

### Enhanced Capabilities (Beyond the Pipeline)

- **SDD Workflow**: Spec extraction → plan gating → decomposition → parallel execution → integration
- **QA Loop Integration**: Automated validation + review cycles with bounded fix attempts and escalation
- **Risk Assessment**: Per-file sensitivity scoring, rollback difficulty estimation, composite risk analysis
- **Semantic Merge Analysis**: Detects call graph breaks, value collisions, and conflicting intent across branches

## Architecture

### Core Principles

The pipeline follows **SOLID** principles:

1. **Single Responsibility**: Each component has one job
   - `CheckpointManager` - Only handles persistence
   - `CostTracker` - Only tracks API costs
   - `TimeoutManager` - Only handles timeouts
   - `TaskQueue` - Only manages task ordering

2. **Open/Closed**: Extensible via traits
   - `PhaseHandler` trait for custom phase implementations
   - Callback system for extensibility

3. **Liskov Substitution**: All phase handlers are interchangeable
   - Any `PhaseHandler` implementation works with the engine

4. **Interface Segregation**: Minimal trait interfaces
   - `PhaseHandler` has only essential methods
   - Optional methods provide additional capabilities

5. **Dependency Inversion**: Depend on abstractions
   - `PipelineOrchestrator` depends on trait objects
   - Components are loosely coupled

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    PipelineOrchestrator                      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                                                         │ │
│  │  ┌───────────┐  ┌──────────┐  ┌──────────────────┐   │ │
│  │  │TaskQueue  │  │Checkpoint│  │  CostTracker     │   │ │
│  │  │           │  │Manager   │  │                  │   │ │
│  │  └───────────┘  └──────────┘  └──────────────────┘   │ │
│  │                                                         │ │
│  │  ┌─────────────────────────────────────────────────┐  │ │
│  │  │            PipelineEngine                        │  │ │
│  │  │  ┌────────────────────────────────────────────┐│  │ │
│  │  │  │  PhaseHandler (Research)                    ││  │ │
│  │  │  │  PhaseHandler (Plan)                        ││  │ │
│  │  │  │  PhaseHandler (Implement)                   ││  │ │
│  │  │  │  PhaseHandler (Review)                      ││  │ │
│  │  │  │  PhaseHandler (Docs)                        ││  │ │
│  │  │  └────────────────────────────────────────────┘│  │ │
│  │  └─────────────────────────────────────────────────┘  │ │
│  │                                                         │ │
│  │  ┌──────────────────┐                                  │ │
│  │  │ TimeoutManager   │                                  │ │
│  │  └──────────────────┘                                  │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. PipelineOrchestrator

High-level coordinator that manages:
- Task submission and queue management
- Checkpoint persistence and recovery
- Cost tracking and budget enforcement
- Timeout management
- Concurrent task execution

**Usage:**

```rust
use d3vx::pipeline::{PipelineOrchestrator, OrchestratorConfig, Task, TaskStatus};

#[tokio::main]
async fn main() {
    // Create orchestrator
    let config = OrchestratorConfig::default()
        .with_max_concurrent_tasks(3)
        .with_checkpoint_dir(".d3vx/checkpoints");
    
    let orchestrator = PipelineOrchestrator::new(config).await?;
    
    // Add tasks
    let task = Task::new("TASK-001", "Implement feature X", "Details...")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::High);
    
    orchestrator.add_task(task).await?;
    
    // Run all queued tasks
    let results = orchestrator.run_all().await?;
    
    println!("Completed {} tasks", results.len());
}
```

### 2. TaskQueue

Priority-based task queue with:
- FIFO ordering within priority levels
- O(1) get_next() operation
- Callback support for notifications

**Priority Levels:**
- `Critical` (4) - Highest priority
- `High` (3) - Important tasks
- `Normal` (2) - Default priority
- `Low` (1) - Background tasks

### 3. CheckpointManager

Crash recovery system that:
- Persists task state to disk
- Enables resume from interruption
- Tracks completed phases
- Supports versioned checkpoints

**Checkpoint Structure:**

```json
{
  "task": { /* Task object */ },
  "completed_phases": [
    ["RESEARCH", { /* PhaseResult */ }],
    ["PLAN", { /* PhaseResult */ }]
  ],
  "created_at": "2024-01-15T10:30:00Z",
  "version": 1
}
```

### 4. CostTracker

API usage tracking with:
- Per-task and session-wide statistics
- Budget enforcement
- Phase-specific tracking
- Model-specific cost calculation

**Cost Estimation:**

```rust
use d3vx::pipeline::estimate_cost;

let cost = estimate_cost("claude-sonnet-4", 1000, 500);
// Returns cost in USD based on model pricing
```

### 5. TimeoutManager

Graceful timeout handling with:
- Configurable per-phase timeouts
- Cancellation token support
- RAII duration tracking
- Cleanup grace periods

**Default Timeouts:**
- Research: 5 minutes
- Plan: 3 minutes
- Implement: 10 minutes
- Review: 4 minutes
- Docs: 3 minutes

### 6. PipelineEngine

Core execution engine that:
- Manages phase handlers
- Executes phase transitions
- Handles retries
- Provides callback system

### 7. Phase Handlers

Each phase has a dedicated handler:

#### ResearchHandler
- Explores codebase structure
- Identifies files to modify
- Documents findings
- **Output:** `.d3vx/research-{task_id}.md`

#### IdeationHandler
- Explores multiple viable approaches
- Evaluates trade-offs (complexity, risk, maintainability)
- Surfaces clarifying questions
- **Output:** Approach comparison with recommendation

#### PlanHandler
- Creates structured implementation plan
- Defines subtasks with dependencies
- **Output:** `.d3vx/plan-{task_id}.json`

#### DraftHandler
- Generates implementation drafts as unified diffs
- **Output:** `.d3vx/draft-{task_id}.diff`

#### ReviewHandler
- Reviews changes for issues
- Fixes problems directly
- Runs verification commands
- **Output:** `REVIEW: APPROVED` or `REVIEW: FIXED`

#### ImplementHandler
- Executes implementation plan
- Updates subtask status
- Commits after each subtask
- **Output:** Source code changes

#### DocsHandler
- Updates documentation
- Adds inline comments
- Updates CHANGELOG
- **Output:** Documentation changes

## Task Lifecycle

```
1. Task Created (BACKLOG)
   ↓
2. Task Queued (QUEUED)
   ↓
3. Research Phase
   - Explore codebase
   - Document findings
   ↓
4. Ideation Phase
   - Explore alternatives
   - Evaluate trade-offs
   - Surface clarifying questions
   ↓
5. Plan Phase
   - Create implementation plan
   - Define subtasks
   ↓
6. Draft Phase
   - Generate implementation drafts
   ↓
7. Review Phase
   - Review changes
   - Fix issues
   ↓
8. Implement Phase
   - Execute plan
   - Make commits
   ↓
9. QA Integration (post-implementation)
   - Run validation (type-check, test, lint)
   - Run automated review
   - Fix cycles (bounded retries)
   ↓
10. Docs Phase
    - Update documentation
    ↓
11. Task Completed (COMPLETED)
```

## SDD (Subagent-Driven Development)

For complex tasks, the SDD workflow decomposes work into parallel child agents:

```
Spec Extraction → Plan Gate → Decomposition → Execution → Integration
```

- **SpecExtractor**: Derives structured spec from user input (deterministic, no LLM)
- **PlanGate**: Hard gate wrapping ApprovalFlow — validates complexity thresholds
- **SddDecomposer**: Groups ExecutionPlan steps by directory, detects dependencies
- **SddExecutor**: Runs children respecting dependency graph (Parallel/Sequential/DepOrder)
- **SddIntegrator**: Merges child results, detects file conflicts, aggregates status

## Enhanced Risk & Merge Analysis

Beyond the linear pipeline, d3vx includes structural analysis:

- **RiskAssessment**: Composite scoring from file sensitivity (40%), rollback difficulty (30%), and dependency breadth (30%)
- **SemanticMergeAnalyzer**: Detects call graph breaks (deleted symbols still referenced), value collisions (enum variant changes), and conflicting intent (same file modified differently by two branches)

## Error Handling

### Phase Errors

```rust
pub enum PhaseError {
    ExecutionFailed { message: String },
    Cancelled,
    Timeout { timeout_ms: u64 },
    InvalidTransition { from: String, to: String },
    ConfigError { message: String },
    IoError { source: std::io::Error },
    AgentError(AgentLoopError),
    NoAgent,
    Other(String),
}
```

### Retry Logic

Tasks can retry failed phases:
- Default: 3 retries
- Configurable per task
- Automatic retry on transient errors

## Best Practices

### 1. Task Creation

```rust
let task = Task::new("AUTH-001", "Add authentication", "Implement JWT auth")
    .with_status(TaskStatus::Queued)
    .with_priority(Priority::High)
    .with_worktree("/path/to/worktree")
    .with_project_root("/project/root");
```

### 2. Context Configuration

```rust
let context = PhaseContext::new(task, "/project/root", "/worktree/path")
    .with_agent_rules("Use strict mode")
    .with_memory_context("Previous session context")
    .with_session_id("session-123");
```

### 3. Orchestrator Setup

```rust
let config = OrchestratorConfig::default()
    .with_max_concurrent_tasks(3)
    .with_checkpoint_dir(".d3vx/checkpoints")
    .without_auto_recovery(); // For manual control

let orchestrator = PipelineOrchestrator::new(config).await?;
orchestrator.set_agent(agent);
```

### 4. Cost Management

```rust
let config = CostTrackerConfig {
    max_task_cost: Some(5.0),      // $5 per task
    max_session_cost: Some(50.0),  // $50 per session
    track_by_phase: true,
    track_by_model: true,
};
```

### 5. Timeout Configuration

```rust
let config = TimeoutConfig::default()
    .with_phase_timeout(Phase::Implement, Duration::from_secs(900)); // 15 minutes
```

## Testing

Each component includes comprehensive tests:

```bash
# Run all pipeline tests
cargo test pipeline::

# Run specific component tests
cargo test checkpoint::
cargo test cost_tracker::
cargo test timeout::
```

## Performance

### Benchmarks

- Task queue insertion: O(log n)
- Checkpoint save: ~1-2ms
- Cost tracking update: O(1)
- Timeout overhead: <1ms

### Scalability

- **Concurrent tasks:** Configurable (default: 3)
- **Queue capacity:** Unlimited (configurable)
- **Checkpoint size:** ~10KB per task
- **Memory overhead:** ~1MB per 100 active tasks

## Monitoring

### Queue Statistics

```rust
let stats = orchestrator.queue_stats().await;
println!("Queued: {}, Active: {}, Completed: {}", 
    stats.queued, stats.in_progress, stats.completed);
```

### Cost Statistics

```rust
let stats = orchestrator.cost_stats().await;
println!("Total cost: ${:.2}", stats.total_cost_usd);
println!("API calls: {}", stats.api_calls);
```

### Active Tasks

```rust
let active = orchestrator.active_tasks().await;
for (task_id, worktree) in active {
    println!("Task {} in {}", task_id, worktree);
}
```

## Integration with Agent

The pipeline integrates with the AgentLoop for LLM execution:

```rust
let agent = AgentLoop::new(provider, tools, config);
orchestrator.set_agent(Arc::new(agent));

// Each phase handler uses the agent to:
// 1. Clear conversation history
// 2. Add phase-specific instruction
// 3. Run the agent
// 4. Extract results
```

## Future Enhancements

See [FEATURE_GAPS.md](../../FEATURE_GAPS.md) for the complete gap analysis.

- [ ] GitHub Issue Auto-Picker — autonomous issue selection and triage
- [ ] PR Auto-Creation & CI Fix Loop — issue to merged PR without human intervention
- [ ] Review Comment Response Loop — agents respond to PR review comments
- [ ] Multi-Project Workspace — manage multiple repos from one instance
- [ ] Cost Budget & Circuit Breaker — per-task kill-switch on budget exceeded
- [ ] Model Routing / Fallback — route simple tasks to fast models, complex to smart
- [ ] Parallel Issue Resolution — run multiple issue pipelines simultaneously
- [ ] Codebase Memory / Embedding Index — semantic code search
- [ ] TDD Enforcement Loop — red/green cycle at the agent level
- [ ] LSP-Aware Agent Context — real-time diagnostic feedback during implementation

## Performance
