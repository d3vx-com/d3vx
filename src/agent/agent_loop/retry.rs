//! Retry logic with exponential backoff and rate-limit handling.

use std::time::Duration;
use tracing::warn;

use crate::providers::{ProviderError, StreamResult};

use super::config::{BASE_DELAY_MS, MAX_RETRIES};
use super::pacing::{GLOBAL_API_SEMAPHORE, LAST_API_REQUEST};
use super::types::AgentEvent;
use super::AgentLoop;

impl AgentLoop {
    /// Send a request with retries for transient errors.
    pub(super) async fn send_with_retry(
        &self,
        request: crate::providers::MessagesRequest,
    ) -> Result<StreamResult, ProviderError> {
        let mut attempt = 0;
        let mut rate_limit_attempt = 0;
        const MAX_429_RETRIES: u32 = 3; // OpenCode parity

        loop {
            attempt += 1;

            // Smart queuing: global pacing and concurrency limit
            let _permit = GLOBAL_API_SEMAPHORE.acquire().await.unwrap();
            {
                let mut last = LAST_API_REQUEST.lock().await;
                let now = std::time::Instant::now();
                let delay_between_requests = Duration::from_millis(500);
                let elapsed = now.duration_since(*last);
                if elapsed < delay_between_requests {
                    let sleep_time = delay_between_requests - elapsed;
                    tokio::time::sleep(sleep_time).await;
                }
                *last = std::time::Instant::now();
            }

            match self.provider.send(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    // Handle 429 Too Many Requests with dedicated backoff
                    if let ProviderError::RateLimited(_) = e {
                        rate_limit_attempt += 1;
                        if rate_limit_attempt <= MAX_429_RETRIES {
                            let delay_ms = e.retry_delay_ms().unwrap_or(60_000);
                            warn!(
                                attempt = rate_limit_attempt,
                                max_retries = MAX_429_RETRIES,
                                delay_ms = delay_ms,
                                error = %e,
                                "Rate limited (429). Applying explicit backoff..."
                            );

                            self.emit(AgentEvent::Error {
                                error: format!(
                                    "Rate limited - retrying in {}s (attempt {}/{})",
                                    delay_ms / 1000,
                                    rate_limit_attempt,
                                    MAX_429_RETRIES
                                ),
                            });

                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            continue;
                        }
                        return Err(e);
                    }

                    // Standard transient error handling
                    if e.is_retryable() && attempt <= MAX_RETRIES {
                        let delay_ms = (BASE_DELAY_MS * 2u64.pow(attempt - 1)).min(32_000);
                        warn!(
                            attempt = attempt,
                            max_retries = MAX_RETRIES,
                            delay_ms = delay_ms,
                            error = %e,
                            "Retrying after transient error"
                        );

                        self.emit(AgentEvent::Error {
                            error: format!(
                                "API error - retrying in {}s (attempt {}/{})",
                                delay_ms / 1000,
                                attempt,
                                MAX_RETRIES
                            ),
                        });

                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }
}
