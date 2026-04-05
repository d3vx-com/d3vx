//! History Reader
//!
//! Provides unified, read-only access to session history for resume, debugging,
//! and inspection purposes.

use crate::store::database::{Database, DatabaseError};
use crate::store::event::EventStore;
use crate::store::message::MessageStore;
use crate::store::session::SessionStore;
use crate::store::{Session, TaskEvent};

/// Bounds for paginated history access.
#[derive(Debug, Clone, Default)]
pub struct HistoryBounds {
    /// Maximum number of results to return.
    pub limit: usize,
    /// Offset from the start (0-based).
    pub offset: usize,
}

impl HistoryBounds {
    /// Create new bounds with limit and offset.
    pub fn new(limit: usize, offset: usize) -> Self {
        Self { limit, offset }
    }

    /// Create bounds with just a limit (offset 0).
    pub fn limit(limit: usize) -> Self {
        Self { limit, offset: 0 }
    }

    /// Create bounds for the last N items.
    pub fn last(limit: usize) -> Self {
        Self { limit, offset: 0 }
    }

    /// Create bounds for a page.
    pub fn page(page: usize, page_size: usize) -> Self {
        Self {
            limit: page_size,
            offset: page * page_size,
        }
    }

    /// Get SQL limit parameter.
    pub fn sql_limit(&self) -> usize {
        self.limit.min(1000)
    }
}

/// Filter criteria for history queries.
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    /// Filter by project path.
    pub project_path: Option<String>,
    /// Filter by task ID.
    pub task_id: Option<String>,
    /// Filter by session state.
    pub session_state: Option<String>,
    /// Filter by time range (start).
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Filter by time range (end).
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl HistoryFilter {
    /// Create a new filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by project path.
    pub fn with_project(mut self, path: impl Into<String>) -> Self {
        self.project_path = Some(path.into());
        self
    }

    /// Filter by task ID.
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Filter by session state.
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.session_state = Some(state.into());
        self
    }

    /// Filter by time range.
    pub fn with_time_range(
        mut self,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        self.from_time = from;
        self.to_time = to;
        self
    }
}

/// A complete history query with bounds and filter.
#[derive(Debug, Clone)]
pub struct HistoryQuery {
    /// What to retrieve.
    pub kind: HistoryKind,
    /// Bounds for pagination.
    pub bounds: HistoryBounds,
    /// Filter criteria.
    pub filter: HistoryFilter,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            kind: HistoryKind::Sessions,
            bounds: HistoryBounds::limit(50),
            filter: HistoryFilter::new(),
        }
    }
}

impl HistoryQuery {
    /// Create a new query.
    pub fn new(kind: HistoryKind, bounds: HistoryBounds, filter: HistoryFilter) -> Self {
        Self {
            kind,
            bounds,
            filter,
        }
    }

    /// Query for sessions.
    pub fn sessions() -> Self {
        Self::default()
    }

    /// Query for events.
    pub fn events() -> Self {
        Self {
            kind: HistoryKind::Events,
            ..Default::default()
        }
    }

    /// Query for recent sessions.
    pub fn recent_sessions(limit: usize) -> Self {
        Self {
            kind: HistoryKind::Sessions,
            bounds: HistoryBounds::limit(limit),
            filter: HistoryFilter::new(),
        }
    }

    /// Query for recent events.
    pub fn recent_events(limit: usize) -> Self {
        Self {
            kind: HistoryKind::Events,
            bounds: HistoryBounds::limit(limit),
            filter: HistoryFilter::new(),
        }
    }

    /// Add a filter.
    pub fn filter(mut self, filter: HistoryFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Set bounds.
    pub fn bounds(mut self, bounds: HistoryBounds) -> Self {
        self.bounds = bounds;
        self
    }

    /// Set kind.
    pub fn kind(mut self, kind: HistoryKind) -> Self {
        self.kind = kind;
        self
    }
}

/// What kind of history to retrieve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    /// Session metadata (not transcript content).
    Sessions,
    /// Task events (internal runtime events).
    Events,
    /// Both sessions and events.
    All,
}

impl std::fmt::Display for HistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistoryKind::Sessions => write!(f, "sessions"),
            HistoryKind::Events => write!(f, "events"),
            HistoryKind::All => write!(f, "all"),
        }
    }
}

/// Statistics about available history.
#[derive(Debug, Clone, Default)]
pub struct HistoryStats {
    /// Total sessions.
    pub total_sessions: usize,
    /// Total events.
    pub total_events: usize,
    /// Oldest session timestamp.
    pub oldest_session: Option<chrono::DateTime<chrono::Utc>>,
    /// Newest session timestamp.
    pub newest_session: Option<chrono::DateTime<chrono::Utc>>,
    /// Sessions by project.
    pub sessions_by_project: std::collections::HashMap<String, usize>,
}

impl HistoryStats {
    /// Get approximate age of oldest session.
    pub fn oldest_session_age(&self) -> Option<chrono::Duration> {
        self.oldest_session.map(|ts| chrono::Utc::now() - ts)
    }

    /// Get approximate age of newest session.
    pub fn newest_session_age(&self) -> Option<chrono::Duration> {
        self.newest_session.map(|ts| chrono::Utc::now() - ts)
    }
}

/// Unified history reader for sessions and events.
///
/// Provides read-only access to:
/// - Session metadata (not full transcript)
/// - Task events (internal runtime events)
pub struct HistoryReader<'a> {
    session_store: &'a SessionStore<'a>,
    #[allow(dead_code)]
    message_store: &'a MessageStore<'a>,
    event_store: &'a EventStore<'a>,
}

impl<'a> HistoryReader<'a> {
    /// Create a new history reader.
    pub fn new(
        _db: &'a Database,
        session_store: &'a SessionStore,
        message_store: &'a MessageStore,
        event_store: &'a EventStore,
    ) -> Self {
        Self {
            session_store,
            message_store,
            event_store,
        }
    }

    /// Get recent sessions.
    pub fn get_recent_sessions(
        &self,
        bounds: &HistoryBounds,
        filter: &HistoryFilter,
    ) -> Result<Vec<Session>, DatabaseError> {
        let options = crate::store::session::SessionListOptions {
            project_path: filter.project_path.clone(),
            task_id: filter.task_id.clone(),
            limit: Some(bounds.sql_limit()),
            offset: Some(bounds.offset),
        };
        self.session_store.list(options)
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>, DatabaseError> {
        self.session_store.get(session_id)
    }

    /// Get the most recent session.
    pub fn get_latest_session(
        &self,
        project_path: Option<&str>,
    ) -> Result<Option<Session>, DatabaseError> {
        self.session_store.get_latest(project_path)
    }

    /// Get recent events.
    pub fn get_recent_events(
        &self,
        bounds: &HistoryBounds,
        filter: &HistoryFilter,
    ) -> Result<Vec<TaskEvent>, DatabaseError> {
        let options = crate::store::event::EventListOptions {
            task_id: filter.task_id.clone(),
            run_id: None,
            event_type: None,
            limit: Some(bounds.sql_limit()),
            offset: Some(bounds.offset),
            descending: true,
        };
        self.event_store.list(options)
    }

    /// Get events for a specific task.
    pub fn get_events_for_task(
        &self,
        task_id: &str,
        bounds: Option<&HistoryBounds>,
    ) -> Result<Vec<TaskEvent>, DatabaseError> {
        let options = crate::store::event::EventListOptions {
            task_id: Some(task_id.to_string()),
            run_id: None,
            event_type: None,
            limit: bounds.map(|b| b.sql_limit()),
            offset: bounds.map(|b| b.offset),
            descending: false,
        };
        self.event_store.list(options)
    }

    /// Get session count.
    pub fn session_count(&self, project_path: Option<&str>) -> Result<i64, DatabaseError> {
        self.session_store.count(project_path)
    }

    /// Get event count for a task.
    pub fn event_count_for_task(&self, task_id: &str) -> Result<i64, DatabaseError> {
        self.event_store.count_for_task(task_id)
    }

    /// Get history statistics.
    pub fn get_stats(&self) -> Result<HistoryStats, DatabaseError> {
        let sessions = self
            .session_store
            .list(crate::store::session::SessionListOptions {
                project_path: None,
                task_id: None,
                limit: Some(1000),
                offset: None,
            })?;

        let mut stats = HistoryStats::default();
        stats.total_sessions = sessions.len();

        let mut sessions_by_project: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for session in &sessions {
            if let Some(ref path) = session.project_path {
                *sessions_by_project.entry(path.clone()).or_insert(0) += 1;
            }

            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&session.created_at) {
                let ts = ts.with_timezone(&chrono::Utc);
                stats.oldest_session = Some(stats.oldest_session.map(|o| o.min(ts)).unwrap_or(ts));
                stats.newest_session = Some(stats.newest_session.map(|n| n.max(ts)).unwrap_or(ts));
            }
        }

        stats.sessions_by_project = sessions_by_project;

        Ok(stats)
    }

    /// Execute a history query.
    pub fn execute(&self, query: &HistoryQuery) -> Result<HistoryResult, DatabaseError> {
        match query.kind {
            HistoryKind::Sessions => {
                let sessions = self.get_recent_sessions(&query.bounds, &query.filter)?;
                Ok(HistoryResult::Sessions(sessions))
            }
            HistoryKind::Events => {
                let events = self.get_recent_events(&query.bounds, &query.filter)?;
                Ok(HistoryResult::Events(events))
            }
            HistoryKind::All => {
                let sessions = self.get_recent_sessions(&query.bounds, &query.filter)?;
                let events = self.get_recent_events(&query.bounds, &query.filter)?;
                Ok(HistoryResult::All { sessions, events })
            }
        }
    }
}

/// Result of a history query.
#[derive(Debug)]
pub enum HistoryResult {
    /// Session metadata.
    Sessions(Vec<Session>),
    /// Task events.
    Events(Vec<TaskEvent>),
    /// Both sessions and events.
    All {
        sessions: Vec<Session>,
        events: Vec<TaskEvent>,
    },
}

impl HistoryResult {
    /// Get session count.
    pub fn session_count(&self) -> usize {
        match self {
            HistoryResult::Sessions(s) => s.len(),
            HistoryResult::All { sessions, .. } => sessions.len(),
            HistoryResult::Events(_) => 0,
        }
    }

    /// Get event count.
    pub fn event_count(&self) -> usize {
        match self {
            HistoryResult::Events(e) => e.len(),
            HistoryResult::All { events, .. } => events.len(),
            HistoryResult::Sessions(_) => 0,
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.session_count() == 0 && self.event_count() == 0
    }
}
