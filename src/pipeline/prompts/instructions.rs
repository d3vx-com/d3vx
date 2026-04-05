//! Phase-specific instruction builders
//!
//! Builds the complete user instruction for each phase by combining
//! the phase-specific instruction with task context.

use super::super::phases::Phase;

/// Build the complete user instruction for a phase.
///
/// Combines the phase-specific instruction with task context.
pub fn build_phase_instruction(
    phase: Phase,
    task_title: &str,
    task_instruction: &str,
    task_id: &str,
    memory_context: Option<&str>,
    agent_rules: Option<&str>,
    ignore_instruction: Option<&str>,
) -> String {
    let mut instruction = String::new();

    // Phase header
    instruction.push_str(&format!("# {} Phase\n\n", phase.label()));

    // Task information
    instruction.push_str("## Task\n");
    instruction.push_str(&format!("**{}**\n\n", task_title));
    instruction.push_str(task_instruction);
    instruction.push_str("\n\n");

    // Memory context if available
    if let Some(memory) = memory_context {
        instruction.push_str("## Context from Previous Sessions\n");
        instruction.push_str(memory);
        instruction.push_str("\n\n");
    }

    // Phase-specific instructions
    instruction.push_str(&get_phase_specific_instructions(phase, task_id));
    instruction.push_str("\n\n");

    // Agent rules if available
    if let Some(rules) = agent_rules {
        instruction.push_str("## Project Rules\n");
        instruction.push_str(rules);
        instruction.push_str("\n\n");
    }

    // Warnings if any
    if let Some(ignore) = ignore_instruction {
        instruction.push_str("## Warnings\n");
        instruction.push_str(ignore);
        instruction.push_str("\n\n");
    }

    instruction
}

/// Get phase-specific instruction details.
pub(crate) fn get_phase_specific_instructions(phase: Phase, task_id: &str) -> String {
    match phase {
        Phase::Research => format!(
            r#"## Your Mission
1. **Explore the codebase** - understand the directory structure, key files, and patterns used
2. **Identify touchpoints** - list every file that will need to be created or modified
3. **Detect risks** - note any existing code that could break, edge cases, or dependencies
4. **Document findings** - write a concise report to `.d3vx/research-{}.md`

## Output Format
Your research file must include:
- Files to modify (with brief reason)
- Files to create (with purpose)
- Key patterns to follow (naming, structure, imports)
- Risks and dependencies
- Recommended approach (1 paragraph)

**STOP after writing the research file. Do NOT implement anything.**"#,
            task_id
        ),
        Phase::Ideation => r#"## Your Mission
1. **Explore alternatives** - identify at least 2 viable approaches
2. **Evaluate trade-offs** - complexity, risk, maintainability for each
3. **Recommend an approach** with clear reasoning
4. **Surface clarifying questions** if critical information is missing

## Output Format
- List each approach with pros, cons, effort, and risk
- Clearly state which you recommend and why
- Ask up to 3 clarifying questions if needed

**STOP after producing the comparison. Do NOT implement anything.**"#
            .to_string(),
        Phase::Plan => format!(
            r#"## Your Mission
1. **Read the research** at `.d3vx/research-{}.md` (if it exists)
2. **Create a Structured Implementation Specification** in JSON format
3. **Write the spec** to `.d3vx/plan-{}.json`

## Spec Format (Strict JSON)
You MUST output a valid JSON file with this structure:
```json
{{
  "summary": "Brief description of what is being implemented",
  "subtasks": [
    {{
      "id": "ST-001",
      "description": "Create the main module",
      "files": ["src/module.rs"],
      "status": "pending",
      "dependencies": []
    }}
  ],
  "files_to_create": [
    {{
      "path": "src/module.rs",
      "purpose": "Main module providing the core functionality"
    }}
  ],
  "files_to_modify": [
    {{
      "path": "src/lib.rs",
      "purpose": "Export the new module publicly"
    }}
  ],
  "api_signatures": ["pub struct NewType {{ ... }}"],
  "acceptance_criteria": ["Feature handles edge case X"],
  "constraints": ["Follow existing patterns in crate Y"],
  "verification": ["cargo check", "cargo test"],
  "risks": ["Potential issue and mitigation"]
}}
```

All fields except `summary` and `subtasks` are optional (omit or use empty arrays).
The spec will be read by the Implement phase agent as its primary context — keep it precise and actionable.

**STOP after writing the JSON spec file. Do NOT implement anything yet.**"#,
            task_id, task_id
        ),
        Phase::Draft => format!(
            r#"## Your Mission
1. **Read the plan** at `.d3vx/plan-{}.json`
2. **Propose changes** as unified diffs for every subtask
3. **Use the `draft_change` tool** to save your proposals
4. **Ensure high quality** and adherence to patterns

**STOP after drafting all changes. Do NOT touch original files.**"#,
            task_id
        ),
        Phase::Implement => format!(
            r#"## Your Mission
1. **Read the implementation spec** at `.d3vx/plan-{}.json`
2. **Implement every pending subtask** in the spec cleanly
3. **Update the JSON spec** by setting subtask `status` to `completed` as you finish them
4. **Follow existing code patterns**
5. **Verify it compiles** with `cargo check`
6. **Make a git commit** with message: `feat: <subtask description>`

## Workflow
For each subtask:
1. Implement the changes
2. Update the spec JSON (set status to "completed")
3. Run verification commands
4. Commit with descriptive message

**Do NOT push or create a PR - the daemon handles that.**"#,
            task_id
        ),
        Phase::Review => format!(
            r#"## Your Mission
1. **Review the git diff** - `git diff HEAD~1` or `git diff main`
2. **Read the plan** at `.d3vx/plan-{}.json` and run any defined verification commands
3. **Check for issues** (Security, Types, Logic, Style)
4. **Fix any issues you find directly**
5. **Run tests** to verify the implementation
6. **Final commit** if you made any fixes

## Verdict
When done, output one of:
- `REVIEW: APPROVED`
- `REVIEW: FIXED`

**Do NOT output REJECTED - fix everything you find.**"#,
            task_id
        ),
        Phase::Docs => r#"## Your Mission
1. **Review the implementation** to understand what was built
2. **Update existing documentation** (README, API docs, etc.)
3. **Create new documentation** if needed
4. **Add inline code comments** where complex logic exists
5. **Update CHANGELOG** if applicable
6. **Final commit** with message: `docs: update documentation for <feature>`

## Documentation Requirements
- All public APIs must be documented
- Complex algorithms need inline comments
- Update examples in README if behavior changed
- Add rustdoc comments to new public functions"#
            .to_string(),
    }
}
