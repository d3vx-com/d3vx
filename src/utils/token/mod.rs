//! Token Utilities Module
//!
//! Provides accurate token counting and context window management.
//! Follows OpenCode's approach:
//! - Uses API-returned token counts when available
//! - Falls back to character-based estimation
//! - Tracks cache tokens separately
//! - Provides context overflow detection

pub mod estimator;
pub mod message_tokens;
pub mod model_limits;
pub mod overflow;

pub use estimator::{estimate_for_code, estimate_for_json, estimate_tokens_for_text};
pub use message_tokens::{estimate_block_tokens, estimate_message_tokens, estimate_tokens};
pub use model_limits::{get_default_limits, get_model_limits, ModelLimits, MODEL_LIMITS};
pub use overflow::{is_context_overflow, ContextOverflowCheck, COMPACTION_BUFFER};
