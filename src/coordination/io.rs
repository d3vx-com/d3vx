//! Atomic JSON / JSONL helpers.
//!
//! All writes go via a sibling tempfile + `rename`. On POSIX `rename` is
//! atomic within the same filesystem, which is what we get for two files
//! in the same directory. Readers therefore never observe a partial
//! document; they see the previous full version or the new full version.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::errors::CoordinationError;

/// Write `value` as pretty JSON to `path` atomically. Overwrites any
/// existing file at `path`.
pub fn atomic_write_json<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), CoordinationError> {
    let path = path.as_ref();
    let tmp = temp_path_for(path);

    let contents = serde_json::to_vec_pretty(value).map_err(|source| {
        CoordinationError::Serialize {
            path: path.to_path_buf(),
            source,
        }
    })?;

    // Create tempfile, write, flush, then rename over the target.
    {
        let mut f = File::create(&tmp).map_err(|source| CoordinationError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.write_all(&contents).map_err(|source| CoordinationError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.sync_all().map_err(|source| CoordinationError::Io {
            path: tmp.clone(),
            source,
        })?;
    }

    std::fs::rename(&tmp, path).map_err(|source| CoordinationError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read JSON at `path`. Returns `Ok(None)` when the file does not exist;
/// other IO errors and malformed JSON surface as errors so a coordination
/// layer that relies on specific file contents fails loudly.
pub fn read_json_if_exists<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<Option<T>, CoordinationError> {
    let path = path.as_ref();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CoordinationError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let value: T =
        serde_json::from_slice(&bytes).map_err(|source| CoordinationError::Deserialize {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(Some(value))
}

/// Create a file exclusively (fails if it exists). Used to implement
/// atomic ownership claims via `O_CREAT|O_EXCL`.
///
/// On success, writes `contents` to the newly-created file.
/// The caller receives `Ok(true)` on success or `Ok(false)` if the file
/// already existed (another party claimed first). Any other IO error
/// surfaces as `Err`.
pub fn create_exclusive(
    path: impl AsRef<Path>,
    contents: &[u8],
) -> Result<bool, CoordinationError> {
    let path = path.as_ref();
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => {
            f.write_all(contents).map_err(|source| CoordinationError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            f.sync_all().map_err(|source| CoordinationError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(CoordinationError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Append one JSON value as a line to a JSONL file. Creates the file if
/// missing.
///
/// Concurrency: the content and terminating newline are packed into a
/// single buffer and flushed via **one** `write_all` call. On POSIX
/// `write(2)` on an `O_APPEND` file descriptor is atomic for buffers up
/// to `PIPE_BUF` (≥512 bytes, 4 KiB on Linux/macOS) — so two agents
/// appending small messages concurrently cannot interleave. Messages
/// larger than `PIPE_BUF` lose this guarantee; we do not enforce a
/// limit, but the coordination protocol's messages are expected to be
/// short JSON envelopes well under that threshold.
pub fn append_jsonl<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), CoordinationError> {
    let path = path.as_ref();
    let mut line = serde_json::to_vec(value).map_err(|source| {
        CoordinationError::Serialize {
            path: path.to_path_buf(),
            source,
        }
    })?;
    line.push(b'\n');

    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CoordinationError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    f.write_all(&line).map_err(|source| CoordinationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Read every line of a JSONL file and parse each as `T`. Returns an
/// empty vec if the file doesn't exist. Stops on the first malformed
/// line so corruption is surfaced, not silently skipped.
pub fn read_jsonl<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<Vec<T>, CoordinationError> {
    let path = path.as_ref();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(CoordinationError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|source| CoordinationError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if line.is_empty() {
            continue;
        }
        let value: T =
            serde_json::from_str(&line).map_err(|source| CoordinationError::Deserialize {
                path: path.to_path_buf(),
                source,
            })?;
        out.push(value);
    }
    Ok(out)
}

/// Truncate a JSONL file (typically an inbox) to zero length.
pub fn truncate_file(path: impl AsRef<Path>) -> Result<(), CoordinationError> {
    let path = path.as_ref();
    // Opening with truncate re-creates the file empty.
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .map_err(|source| CoordinationError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn temp_path_for(target: &Path) -> PathBuf {
    let stem = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("coord");
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    // Include PID + nanos to avoid collision between concurrent writers
    // on the same target (which shouldn't happen for a single task id,
    // but costs us nothing to be robust against).
    let suffix = format!(
        ".{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    parent.join(format!("{stem}{suffix}"))
}
