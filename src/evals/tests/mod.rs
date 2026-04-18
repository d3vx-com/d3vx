//! Test suite for the eval harness.
//!
//! Split one file per concern so no individual test module exceeds the
//! project's 300-line guideline.

mod agent_loop_driver_tests;
mod environment_tests;
mod grader_tests;
mod result_tests;
mod runner_helpers;
mod runner_tests;
mod task_tests;
