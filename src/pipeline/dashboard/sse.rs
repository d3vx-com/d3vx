//! SSE (Server-Sent Events) endpoint
//!
//! Streams dashboard events to connected browsers in real-time using
//! the broadcast channel from the parent Dashboard struct.

use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::{format_sse_event, Dashboard};

/// SSE endpoint: streams all dashboard events to the client.
///
/// The connection stays open until the client disconnects. Missed events
/// (slow consumers) are silently dropped via the broadcast channel's
/// lagging behavior.
pub async fn events_stream(
    axum::extract::State(dashboard): axum::extract::State<Dashboard>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = dashboard.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let sse_data = format_sse_event(&event);
            let cleaned = sse_data.trim_end_matches('\n');
            Some(Ok(Event::default().data(cleaned)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::dashboard::{DashboardConfig, DashboardEvent};

    #[tokio::test]
    async fn test_subscribe_receives_events() {
        let db = std::sync::Arc::new(parking_lot::Mutex::new(
            crate::store::Database::in_memory().unwrap(),
        ));
        let dash = Dashboard::new(DashboardConfig {
            enabled: true,
            ..Default::default()
        }, db);

        let mut rx = dash.subscribe();
        dash.broadcast(DashboardEvent::TaskCompleted {
            id: "T-5".into(),
            success: true,
        });

        let event = rx.try_recv().unwrap();
        match event {
            DashboardEvent::TaskCompleted { id, success } => {
                assert_eq!(id, "T-5");
                assert!(success);
            }
            _ => panic!("Expected TaskCompleted"),
        }
    }
}
