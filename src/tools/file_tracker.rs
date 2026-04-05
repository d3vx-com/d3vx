//! File Read Tracker
//!
//! Tracks files read by the Read tool to detect stale edits.
//! When the LLM tries to edit a file that was modified since it last read it,
//! we reject the edit and tell it to re-read the file.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Metadata recorded when a file is read.
#[derive(Debug, Clone)]
pub struct FileReadEntry {
    /// When the file was read.
    pub read_at: SystemTime,
    /// Hash of the file content at read time.
    pub content_hash: u64,
    /// Number of lines in the file at read time.
    pub line_count: usize,
}

/// Result of checking whether a file is stale relative to its last read.
#[derive(Debug, Clone, PartialEq)]
pub enum StaleStatus {
    /// File content matches what was read — safe to edit.
    Fresh,
    /// File was modified since it was last read.
    Stale { modified_since: Duration },
    /// File was never read (or cannot be read now).
    NeverRead,
}

/// Thread-safe tracker for file reads, used to detect stale edits.
#[derive(Debug, Clone)]
pub struct FileReadTracker {
    inner: Arc<Mutex<HashMap<PathBuf, FileReadEntry>>>,
}

impl FileReadTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record that a file was read with the given content.
    pub fn record_read(&self, path: &Path, content: &str) {
        let entry = FileReadEntry {
            read_at: SystemTime::now(),
            content_hash: hash_content(content),
            line_count: content.lines().count(),
        };

        if let Ok(mut map) = self.inner.lock() {
            map.insert(path.to_path_buf(), entry);
        }
    }

    /// Check whether the file has changed since it was last read.
    ///
    /// Re-reads the file from disk and compares the content hash to the stored
    /// hash. Returns `NeverRead` if the file was never tracked or if it cannot
    /// be read now.
    pub fn is_stale(&self, path: &Path) -> StaleStatus {
        let entry = {
            let map = match self.inner.lock() {
                Ok(m) => m,
                Err(_) => return StaleStatus::NeverRead,
            };
            match map.get(path) {
                Some(e) => e.clone(),
                None => return StaleStatus::NeverRead,
            }
        };

        let current_content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return StaleStatus::NeverRead,
        };

        let current_hash = hash_content(&current_content);
        if current_hash == entry.content_hash {
            return StaleStatus::Fresh;
        }

        let modified_since = SystemTime::now()
            .duration_since(entry.read_at)
            .unwrap_or(Duration::ZERO);

        StaleStatus::Stale { modified_since }
    }

    /// Remove all tracked files.
    pub fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }

    /// Remove a single file from tracking.
    pub fn remove(&self, path: &Path) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(path);
        }
    }
}

impl Default for FileReadTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a deterministic hash of file content.
fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(content.as_bytes());
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_record_and_check_fresh() {
        let tracker = FileReadTracker::new();
        let dir = std::env::temp_dir().join("d3vx_tracker_test_fresh");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("fresh.txt");

        let content = "hello world\nline two\n";
        fs::write(&file, content).unwrap();

        tracker.record_read(&file, content);
        assert_eq!(tracker.is_stale(&file), StaleStatus::Fresh);

        // The entry should have the correct metadata
        let map = tracker.inner.lock().unwrap();
        let entry = map.get(&file).unwrap();
        assert_eq!(entry.line_count, 2);
        assert_eq!(entry.content_hash, hash_content(content));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_detects_stale() {
        let tracker = FileReadTracker::new();
        let dir = std::env::temp_dir().join("d3vx_tracker_test_stale");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("stale.txt");

        let original = "original content\n";
        fs::write(&file, original).unwrap();
        tracker.record_read(&file, original);

        // Modify file
        fs::write(&file, "modified content\n").unwrap();

        match tracker.is_stale(&file) {
            StaleStatus::Stale { .. } => {} // expected
            other => panic!("Expected Stale, got {:?}", other),
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_never_read() {
        let tracker = FileReadTracker::new();
        let file = std::env::temp_dir().join("d3vx_never_read_test.txt");

        // File does not exist at all
        assert_eq!(tracker.is_stale(&file), StaleStatus::NeverRead);

        // File exists but was never tracked
        fs::write(&file, "some content").unwrap();
        assert_eq!(tracker.is_stale(&file), StaleStatus::NeverRead);

        fs::remove_file(&file).ok();
    }

    #[test]
    fn test_clear() {
        let tracker = FileReadTracker::new();
        let file = std::env::temp_dir().join("d3vx_clear_test.txt");

        fs::write(&file, "content").unwrap();
        tracker.record_read(&file, "content");
        assert_eq!(tracker.is_stale(&file), StaleStatus::Fresh);

        tracker.clear();
        assert_eq!(tracker.is_stale(&file), StaleStatus::NeverRead);

        fs::remove_file(&file).ok();
    }

    #[test]
    fn test_remove() {
        let tracker = FileReadTracker::new();
        let file = std::env::temp_dir().join("d3vx_remove_test.txt");

        fs::write(&file, "content").unwrap();
        tracker.record_read(&file, "content");
        assert_eq!(tracker.is_stale(&file), StaleStatus::Fresh);

        tracker.remove(&file);
        assert_eq!(tracker.is_stale(&file), StaleStatus::NeverRead);

        fs::remove_file(&file).ok();
    }

    #[test]
    fn test_deleted_file_returns_never_read() {
        let tracker = FileReadTracker::new();
        let dir = std::env::temp_dir().join("d3vx_tracker_test_deleted");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("deleted.txt");

        fs::write(&file, "content").unwrap();
        tracker.record_read(&file, "content");

        // Delete the file
        fs::remove_file(&file).unwrap();
        assert_eq!(tracker.is_stale(&file), StaleStatus::NeverRead);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_hash_deterministic() {
        let content = "same content\nmultiple lines\n";
        let h1 = hash_content(content);
        let h2 = hash_content(content);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_content_different_hash() {
        let h1 = hash_content("content A");
        let h2 = hash_content("content B");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(FileReadTracker::new());
        let dir = std::env::temp_dir().join("d3vx_tracker_test_threads");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("threaded.txt");

        fs::write(&file, "initial").unwrap();

        let mut handles = vec![];
        for i in 0..4 {
            let t = Arc::clone(&tracker);
            let f = file.clone();
            handles.push(thread::spawn(move || {
                let content = format!("thread-{}", i);
                t.record_read(&f, &content);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // At least one entry must exist
        let map = tracker.inner.lock().unwrap();
        assert!(map.contains_key(&file));

        fs::remove_dir_all(&dir).ok();
    }
}
