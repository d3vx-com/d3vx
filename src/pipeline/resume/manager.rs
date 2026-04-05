//! Session Resume Manager
//!
//! Manages session snapshots for resume capability.

use std::path::PathBuf;

use chrono::Utc;
use tokio::fs;
use tracing::{debug, info, warn};

use super::types::{ResumeError, SessionSnapshot, Snapshot, SnapshotInfo};

/// Manages session snapshots for resume capability.
///
/// Snapshots are persisted as JSON files under `snapshot_dir`, one per
/// session, using the file name `{session_id}.json`.
///
/// # Snapshot Types
///
/// The manager supports two snapshot types:
/// - `Snapshot::Full(SessionSnapshot)` - traditional full snapshot
/// - `Snapshot::Compact(CompactResume)` - compact resume with boundary + tail events
///
/// Use `save_snapshot()` for full snapshots (backward compatible).
/// Use `save_compact_resume()` for compact resumes (compaction-aware).
pub struct ResumeManager {
    snapshot_dir: PathBuf,
}

impl ResumeManager {
    /// Create a new manager that reads / writes snapshots in `snapshot_dir`.
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self { snapshot_dir }
    }

    /// Ensure the snapshot directory exists.
    pub async fn initialize(&self) -> Result<(), ResumeError> {
        fs::create_dir_all(&self.snapshot_dir).await?;
        info!("ResumeManager initialized at {:?}", self.snapshot_dir);
        Ok(())
    }

    /// Persist a snapshot to disk atomically using temp file + rename.
    pub async fn save_snapshot(&self, snapshot: &SessionSnapshot) -> Result<(), ResumeError> {
        let snapshot = Snapshot::Full(snapshot.clone());
        self.save(&snapshot).await
    }

    /// Persist a compact resume to disk atomically using temp file + rename.
    ///
    /// This saves a compaction-aware snapshot with boundary summary and tail events,
    /// which is more efficient for resuming long sessions.
    pub async fn save_compact_resume(
        &self,
        compact: &super::compaction::CompactResume,
    ) -> Result<(), ResumeError> {
        let snapshot = Snapshot::Compact(compact.clone());
        self.save(&snapshot).await
    }

    /// Internal save method that handles any Snapshot type.
    async fn save(&self, snapshot: &Snapshot) -> Result<(), ResumeError> {
        let path = self.snapshot_path(snapshot.session_id());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(snapshot)?;

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json.as_bytes()).await?;
        tokio::fs::rename(&temp_path, &path).await?;

        debug!("Saved snapshot for session {}", snapshot.session_id());
        Ok(())
    }

    /// Load the latest snapshot for a given session.
    ///
    /// Returns `Snapshot` which may be either `Full` or `Compact` depending on
    /// what was saved. For backward compatibility, also loads old `SessionSnapshot`
    /// format files.
    pub async fn load_snapshot(&self, session_id: &str) -> Result<Option<Snapshot>, ResumeError> {
        let path = self.snapshot_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path).await?;
        let snapshot: Snapshot = serde_json::from_str(&contents)?;
        debug!(
            "Loaded snapshot for session {} (type: {:?})",
            session_id,
            snapshot.is_compact()
        );
        Ok(Some(snapshot))
    }

    /// Load the latest snapshot for a given session as a `SessionSnapshot`.
    ///
    /// This convenience method converts either full or compact snapshots to the
    /// traditional `SessionSnapshot` format, enabling the restore flow to work
    /// with both snapshot types seamlessly.
    pub async fn load_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionSnapshot>, ResumeError> {
        match self.load_snapshot(session_id).await? {
            Some(snapshot) => Ok(Some(snapshot.to_session_snapshot())),
            None => Ok(None),
        }
    }

    /// Load the latest snapshot for a task by scanning available snapshots.
    ///
    /// Returns the most recent snapshot whose `task_id` matches, converted to
    /// `SessionSnapshot` format if it was stored as a compact resume.
    pub async fn load_snapshot_for_task(
        &self,
        task_id: &str,
    ) -> Result<Option<SessionSnapshot>, ResumeError> {
        let snapshots = self.list_snapshots().await?;
        let mut best: Option<SessionSnapshot> = None;
        let mut best_time: Option<chrono::DateTime<Utc>> = None;

        for info in snapshots {
            if info.task_id == task_id {
                match self.load_session_snapshot(&info.session_id).await {
                    Ok(Some(snapshot)) => match best_time {
                        None => {
                            best = Some(snapshot);
                            best_time = Some(info.snapshot_at);
                        }
                        Some(t) if info.snapshot_at > t => {
                            best = Some(snapshot);
                            best_time = Some(info.snapshot_at);
                        }
                        _ => {}
                    },
                    Ok(None) => {}
                    Err(e) => warn!("Failed to load snapshot {}: {}", info.session_id, e),
                }
            }
        }
        Ok(best)
    }

    /// List metadata for all available snapshots.
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>, ResumeError> {
        let mut infos = Vec::new();
        if !self.snapshot_dir.exists() {
            return Ok(infos);
        }
        let mut entries = fs::read_dir(&self.snapshot_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                match fs::read_to_string(&path).await {
                    Ok(contents) => {
                        if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&contents) {
                            let info = Self::snapshot_to_info(&snapshot);
                            infos.push(info);
                        } else if let Ok(snap) = serde_json::from_str::<SessionSnapshot>(&contents)
                        {
                            infos.push(SnapshotInfo {
                                session_id: snap.session_id,
                                task_id: snap.task_id,
                                snapshot_at: snap.snapshot_at,
                                message_count: snap.messages.len(),
                                file_count: snap.modified_files.len(),
                                event_count: snap.event_log.as_ref().map(|l| l.len()).unwrap_or(0),
                            });
                        } else {
                            warn!("Failed to parse snapshot {:?}", path);
                        }
                    }
                    Err(e) => warn!("Failed to read snapshot {:?}: {}", path, e),
                }
            }
        }
        infos.sort_by(|a, b| b.snapshot_at.cmp(&a.snapshot_at));
        Ok(infos)
    }

    fn snapshot_to_info(snapshot: &Snapshot) -> SnapshotInfo {
        match snapshot {
            Snapshot::Full(s) => SnapshotInfo {
                session_id: s.session_id.clone(),
                task_id: s.task_id.clone(),
                snapshot_at: s.snapshot_at,
                message_count: s.messages.len(),
                file_count: s.modified_files.len(),
                event_count: s.event_log.as_ref().map(|l| l.len()).unwrap_or(0),
            },
            Snapshot::Compact(c) => SnapshotInfo {
                session_id: c.boundary.session_id.clone(),
                task_id: c.boundary.task_id.clone(),
                snapshot_at: c.boundary.created_at,
                message_count: c.boundary.recent_messages.len(),
                file_count: c.boundary.modified_files.len(),
                event_count: c.tail_events.len() + c.messages_before_compaction,
            },
        }
    }

    /// Delete snapshots older than `max_age_days`, returning the count removed.
    pub async fn cleanup_old_snapshots(&self, max_age_days: u64) -> Result<usize, ResumeError> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let infos = self.list_snapshots().await?;
        let mut removed = 0;
        for info in infos {
            if info.snapshot_at < cutoff {
                if self.delete_snapshot(&info.session_id).await? {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    /// Delete the snapshot for a specific session. Returns `true` if a file was removed.
    pub async fn delete_snapshot(&self, session_id: &str) -> Result<bool, ResumeError> {
        let path = self.snapshot_path(session_id);
        if path.exists() {
            fs::remove_file(&path).await?;
            debug!("Deleted snapshot for session {}", session_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn snapshot_path(&self, session_id: &str) -> PathBuf {
        self.snapshot_dir.join(format!("{}.json", session_id))
    }
}
