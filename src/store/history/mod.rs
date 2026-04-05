//! History Reader Module
//!
//! Provides unified, read-only access to session history for resume, debugging,
//! and inspection purposes. This module separates:
//!
//! - **Visible Transcript**: User-visible conversation (messages, summaries)
//! - **Internal Events**: Runtime events for debugging/resume (task events, internal logs)
//!
//! # Design Principles
//!
//! - Read-only: No mutations through this interface
//! - Bounded: All queries support pagination
//! - Separated: Clear distinction between transcript and events
//! - Unified: Single entry point for history access

pub mod reader;
pub mod transcript;

pub use reader::{
    HistoryBounds, HistoryFilter, HistoryKind, HistoryQuery, HistoryReader, HistoryResult,
    HistoryStats,
};
pub use transcript::{TranscriptEntry, TranscriptReader, TranscriptRole, TranscriptSummary};

#[cfg(test)]
mod tests;
