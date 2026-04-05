//! Background Daemon Workers
//!
//! Pre-defined workers that execute on schedule for codebase maintenance.
//! Each worker implements the [`DaemonWorker`] trait and is registered with
//! the [`DaemonScheduler`] which handles interval-based scheduling.

pub mod scheduler;
pub mod workers;

pub use scheduler::{DaemonScheduler, WorkerInfo};
pub use workers::{BuiltinWorkers, DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};
