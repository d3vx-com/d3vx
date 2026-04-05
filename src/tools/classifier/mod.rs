//! Bash Safety Classifier
//!
//! Classifies bash commands by risk level for permission handling.
//! Inspired by Claude Code's BASH_CLASSIFIER feature.

mod patterns;
mod classifier;

pub use classifier::{BashSafetyLevel, BashClassifier};
pub use patterns::SAFETY_PATTERNS;
