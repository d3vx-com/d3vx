//! System-prompt preamble that teaches an agent how to use the
//! coordination tools.
//!
//! The preamble is deliberately short, imperative, and mentions every
//! coordination tool by name so an agent scanning for "can I use…?"
//! finds the name quickly. It does not try to *explain* the protocol at
//! length — longer prompts compete with the task instruction for
//! attention.
//!
//! The preamble takes no parameters — each tool call carries the
//! calling agent's id via `ToolContext.session_id`, so the agent
//! doesn't need to embed its own id in the prompt.

/// Build a coordination preamble to prepend to an agent's system
/// prompt. Stable across agents; call once and reuse.
pub fn coordination_preamble() -> String {
    "# Coordination context\n\
     \n\
     You share a coordination space with other agents. Use these tools\n\
     to cooperate:\n\
     \n\
     - `coord_list_ready_tasks` — see tasks available to claim right now\n\
       (dependencies satisfied, no current owner). Use this before doing\n\
       any new work so you don't duplicate someone else's effort.\n\
     - `coord_claim_task(task_id)` — atomically claim a ready task. Only\n\
       call this after inspecting the task; one agent wins the race.\n\
     - `coord_complete_task(task_id, result)` — mark your claimed task\n\
       done with a short result summary. Call this even for partial\n\
       progress you intend to hand off.\n\
     - `coord_send_message(to, body)` — point-to-point message to\n\
       another agent's inbox. Use for direct coordination, not\n\
       broadcast announcements.\n\
     - `coord_drain_inbox` — read and clear every message addressed to\n\
       you. Call this at the start of each iteration so you don't miss\n\
       instructions.\n\
     \n\
     Conventions:\n\
     1. Check the inbox before starting new work.\n\
     2. Claim before you work; complete before you stop.\n\
     3. When stuck, send a message to the coordinator rather than\n\
        spinning on the same approach.\n"
        .to_string()
}
