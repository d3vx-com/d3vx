//! Inter-agent coordination primitives.
//!
//! A **coordination root** is a directory two or more agent processes
//! share. It exposes a task board (what to do, who owns it, what's
//! done) and per-agent inboxes (point-to-point messages) plus a shared
//! broadcast log.
//!
//! # Design
//!
//! Three strict design choices:
//!
//! 1. **Filesystem-only.** No server, no message queue, no new crate.
//!    Works across processes, across worktrees, and lets an operator
//!    `cat` the coordination state while agents run.
//! 2. **Atomic writes.** Every file update goes through
//!    write-to-tempfile + rename so concurrent readers never see a
//!    torn document. See [`io`].
//! 3. **Cooperative, not adversarial.** Two agents racing to claim a
//!    task use `O_CREAT|O_EXCL` on the claim file (POSIX-atomic),
//!    so the primitive IS correct under concurrency — but the rest of
//!    the coordination model assumes agents are cooperating, not
//!    trying to sabotage each other.
//!
//! # Submodules
//!
//! | Submodule   | Owns                                                  |
//! |-------------|-------------------------------------------------------|
//! | [`io`]      | Atomic JSON/JSONL helpers                             |
//! | [`errors`]  | `CoordinationError` — single error type               |
//! | [`board`]   | Task board (add/claim/complete/list)                  |
//! | [`inbox`]   | Per-agent inbox + shared broadcast                    |

pub mod agent_tools;
pub mod board;
pub mod errors;
pub mod inbox;
pub mod io;
pub mod prompt;
pub mod task;
mod tool_impls;

#[cfg(test)]
mod tests;

pub use agent_tools::CoordinationToolset;
pub use board::CoordinationBoard;
pub use errors::CoordinationError;
pub use inbox::{BroadcastLog, Inbox, Message};
pub use prompt::coordination_preamble;
pub use task::{BoardTask, NewTask, TaskStatus};
