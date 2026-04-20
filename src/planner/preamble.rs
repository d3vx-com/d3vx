//! The system-prompt block that teaches the model to decide phases.
//!
//! This preamble is prepended to the agent's system prompt on the first
//! turn of every user message. It is *the* intellectual core of the
//! planner feature — the whole "autonomous phase selection" product
//! sentence lives or dies on the quality of this prompt.
//!
//! Design decisions:
//!
//! - **Examples over rules.** The model calibrates to the distribution
//!   of examples much better than it follows prescriptive prose. Each
//!   example pairs a prompt with the expected phase list and a short
//!   rationale.
//! - **Bias toward fewer phases.** Adding phases is cheap for the model
//!   to recommend and expensive for the user to sit through. The
//!   default bias is "use the smallest set that actually helps."
//! - **Resume is part of the decision.** If a plan file already exists
//!   for the current task, the model includes it in the decision so
//!   the next turn picks up from the first unchecked section instead
//!   of replanning from scratch.
//! - **Fenced output.** A single ```d3vx-decision``` code block is the
//!   only machine-parsed surface, leaving the rest of the response
//!   free for normal conversational framing.
//!
//! The string is built once at startup; keep it under ~3 KB so it does
//! not meaningfully compete with user context on every turn.

/// Returns the planner preamble — prepend this to the agent's system
/// prompt on every turn where a planning decision may be needed.
pub fn planner_preamble() -> &'static str {
    PLANNER_PREAMBLE
}

const PLANNER_PREAMBLE: &str = r#"# Planner

You decide on your own how much structure this task needs. The seven
phases available are: research, ideation, plan, draft, review,
implement, docs. You choose a *subset* (possibly empty) and an order.

Bias toward fewer phases. Each added phase is latency the user pays.

## How to choose

- Trivial questions, lookups, one-line tweaks, obvious renames: no
  phases. Just answer.
- Single mechanical change in an area you can read in one pass:
  `[implement]`.
- Something that needs a plan before coding, or where "what to build"
  is the hard part: `[plan, implement]` or add `research` upfront.
- Genuinely ambiguous, cross-cutting, or risky work: use the full
  pipeline or most of it.

If `.d3vx/plans/<slug>.md` already exists for this task, list its id
in `resume:` and pick the remaining phases — do not replan sections
that are already checked off.

## Output format

Emit a single fenced block named `d3vx-decision`, then continue your
normal response. The block is parsed; the rest is shown to the user.

```d3vx-decision
phases: [plan, implement]
reason: non-trivial refactor touching multiple modules
resume: null
```

`phases` is a list; empty list `[]` means "direct answer, no
structure". `reason` is one short sentence. `resume` is null or a plan
id like `2026-04-20-thumbnail-cache`.

## Examples

Prompt: "what does the `AgentLoop` struct do?"
```d3vx-decision
phases: []
reason: explanation question, no code change
resume: null
```

Prompt: "rename `foo_bar` to `foo_baz` across the repo"
```d3vx-decision
phases: [implement]
reason: mechanical rename, scope is obvious
resume: null
```

Prompt: "add a --dry-run flag to the daemon command"
```d3vx-decision
phases: [implement]
reason: small scoped change in one command handler
resume: null
```

Prompt: "refactor the tool coordinator so tools can share state"
```d3vx-decision
phases: [plan, implement]
reason: design is the hard part; needs a plan before coding
resume: null
```

Prompt: "replace our retry logic with exponential backoff; the tests
are flaky and I'm not sure which callers rely on the current timing"
```d3vx-decision
phases: [research, plan, implement]
reason: unclear blast radius; inspect callers before deciding shape
resume: null
```

Prompt: "build a notification system with Telegram, email, and
in-app delivery and wire it into the pipeline lifecycle events"
```d3vx-decision
phases: [research, plan, draft, review, implement, docs]
reason: new subsystem, multiple surfaces, needs full pipeline
resume: null
```

Prompt: "continue the thumbnail cache work"
```d3vx-decision
phases: [implement]
reason: resuming existing plan; only Implement remains
resume: 2026-04-20-thumbnail-cache
```
"#;
