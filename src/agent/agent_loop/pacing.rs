//! Global API rate limiting and concurrency control.

use once_cell::sync::Lazy;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

/// Global API semaphore limits concurrent requests (e.g. max 5 concurrent requests).
pub(super) static GLOBAL_API_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(5));

/// Global API pacing timer (guarantees at least 500ms between request starts).
pub(super) static LAST_API_REQUEST: Lazy<Mutex<Instant>> =
    Lazy::new(|| Mutex::new(Instant::now() - Duration::from_secs(1)));
