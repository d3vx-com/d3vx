//! Embedded Web Dashboard
//!
//! Axum HTTP server with SSE real-time updates and a React SPA frontend.
//! The dashboard queries real data from the SQLite store.
//!
//! Frontend is built with Vite + React and embedded at compile time.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub mod api;
pub mod sse;
pub mod static_assets;
pub mod types;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Dashboard server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Listen host (default `127.0.0.1`).
    pub host: String,
    /// Listen port (default `9876`).
    pub port: u16,
    /// Whether the dashboard is enabled.
    pub enabled: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9876,
            enabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// SSE event broadcast to dashboard clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DashboardEvent {
    TaskCreated {
        id: String,
        title: String,
    },
    TaskStatusChanged {
        id: String,
        status: String,
        phase: String,
    },
    TaskCompleted {
        id: String,
        success: bool,
    },
    AgentActivity {
        task_id: String,
        state: String,
    },
    CostUpdate {
        task_id: String,
        cost_usd: f64,
        tokens: u64,
    },
    SystemStatus {
        active_tasks: usize,
        queue_size: usize,
        cost_today: f64,
    },
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors thrown by dashboard operations.
#[derive(Debug, thiserror::Error)]
pub enum DashboardError {
    #[error("Dashboard error: {0}")]
    Server(String),
    #[error("Broadcast error: {0}")]
    Broadcast(String),
}

// ---------------------------------------------------------------------------
// Dashboard server
// ---------------------------------------------------------------------------

const BROADCAST_CAPACITY: usize = 256;

/// Dashboard server that broadcasts events via SSE and serves a REST API
/// backed by the SQLite database.
#[derive(Clone)]
pub struct Dashboard {
    config: DashboardConfig,
    tx: broadcast::Sender<DashboardEvent>,
    db: Arc<parking_lot::Mutex<crate::store::Database>>,
}

impl Dashboard {
    /// Create a new dashboard with the given configuration and database.
    pub fn new(config: DashboardConfig, db: Arc<parking_lot::Mutex<crate::store::Database>>) -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        info!(
            "Dashboard initialized (host={}, port={}, enabled={})",
            config.host, config.port, config.enabled
        );
        Self { config, tx, db }
    }

    /// Create a disabled dashboard (no-op).
    pub fn disabled() -> Self {
        Self::new(
            DashboardConfig {
                enabled: false,
                ..Default::default()
            },
            Arc::new(parking_lot::Mutex::new(
                crate::store::Database::in_memory()
                    .expect("Failed to create dummy database"),
            )),
        )
    }

    /// Broadcast an event to all connected SSE clients.
    pub fn broadcast(&self, event: DashboardEvent) {
        if !self.config.enabled {
            debug!("Dashboard disabled -- dropping event {:?}", event);
            return;
        }
        let receivers = self.tx.receiver_count();
        if receivers == 0 {
            debug!("No dashboard clients connected -- dropping event");
            return;
        }
        match self.tx.send(event) {
            Ok(n) => debug!("Event delivered to {} clients", n),
            Err(_) => warn!("Failed to broadcast dashboard event (no receivers)"),
        }
    }

    /// Get a new receiver for SSE events.
    pub fn subscribe(&self) -> broadcast::Receiver<DashboardEvent> {
        self.tx.subscribe()
    }

    /// Number of currently connected receivers.
    pub fn client_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// SSE endpoint URL.
    pub fn sse_url(&self) -> String {
        format!(
            "http://{}:{}/api/events",
            self.config.host, self.config.port
        )
    }

    /// Dashboard landing page URL.
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &DashboardConfig {
        &self.config
    }

    /// Return a reference to the database handle.
    pub fn db(&self) -> &Arc<parking_lot::Mutex<crate::store::Database>> {
        &self.db
    }

    /// Start the HTTP server with axum.
    pub async fn serve(&self) -> Result<(), DashboardError> {
        if !self.config.enabled {
            debug!("Dashboard not enabled -- serve() is a no-op");
            return Ok(());
        }
        let app = api::create_router(self.clone());
        let addr = format!("{}:{}", self.config.host, self.config.port);
        info!("Dashboard HTTP server starting on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| DashboardError::Server(format!("Bind failed: {}", e)))?;
        info!(
            "Dashboard listening at http://{}:{}",
            self.config.host, self.config.port
        );
        axum::serve(listener, app)
            .await
            .map_err(|e| DashboardError::Server(format!("Server error: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serialize a dashboard event as an SSE `data:` line.
pub fn format_sse_event(event: &DashboardEvent) -> String {
    let json = serde_json::to_string(event).unwrap_or_else(|e| {
        format!(
            r#"{{"type":"error","message":"serialization failed: {}"}}"#,
            e
        )
    });
    format!("data: {}\n\n", json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Arc<parking_lot::Mutex<crate::store::Database>> {
        Arc::new(parking_lot::Mutex::new(
            crate::store::Database::in_memory()
                .expect("Failed to create test database"),
        ))
    }

    #[test]
    fn test_dashboard_config_default() {
        let cfg = DashboardConfig::default();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 9876);
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_dashboard_urls() {
        let dash = Dashboard::new(
            DashboardConfig {
                enabled: true,
                host: "0.0.0.0".into(),
                port: 8080,
            },
            test_db(),
        );
        assert_eq!(dash.url(), "http://0.0.0.0:8080");
        assert_eq!(dash.sse_url(), "http://0.0.0.0:8080/api/events");
    }

    #[test]
    fn test_format_sse_event_task_created() {
        let event = DashboardEvent::TaskCreated {
            id: "T-1".into(),
            title: "Fix bug".into(),
        };
        let sse = format_sse_event(&event);
        assert!(sse.starts_with("data: "));
        assert!(sse.ends_with("\n\n"));
        assert!(sse.contains("task_created"));
        assert!(sse.contains("T-1"));
    }

    #[test]
    fn test_format_sse_event_system_status() {
        let event = DashboardEvent::SystemStatus {
            active_tasks: 3,
            queue_size: 1,
            cost_today: 0.42,
        };
        let sse = format_sse_event(&event);
        assert!(sse.contains("system_status"));
        assert!(sse.contains("0.42"));
    }

    #[tokio::test]
    async fn test_broadcast_to_subscriber() {
        let dash = Dashboard::new(
            DashboardConfig {
                enabled: true,
                ..Default::default()
            },
            test_db(),
        );
        let mut rx = dash.subscribe();
        dash.broadcast(DashboardEvent::TaskCreated {
            id: "T-42".into(),
            title: "Do stuff".into(),
        });
        let event = rx.try_recv().unwrap();
        match event {
            DashboardEvent::TaskCreated { id, title } => {
                assert_eq!(id, "T-42");
                assert_eq!(title, "Do stuff");
            }
            _ => panic!("Expected TaskCreated event"),
        }
    }

    #[tokio::test]
    async fn test_broadcast_disabled_drops_event() {
        let dash = Dashboard::new(
            DashboardConfig {
                enabled: false,
                ..Default::default()
            },
            test_db(),
        );
        let mut rx = dash.subscribe();
        dash.broadcast(DashboardEvent::TaskCreated {
            id: "T-1".into(),
            title: "Ignored".into(),
        });
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_client_count() {
        let dash = Dashboard::new(
            DashboardConfig {
                enabled: true,
                ..Default::default()
            },
            test_db(),
        );
        assert_eq!(dash.client_count(), 0);
        let _rx1 = dash.subscribe();
        assert_eq!(dash.client_count(), 1);
        let _rx2 = dash.subscribe();
        assert_eq!(dash.client_count(), 2);
    }

    #[test]
    fn test_disabled_convenience() {
        let dash = Dashboard::disabled();
        assert!(!dash.config().enabled);
    }

    #[tokio::test]
    async fn test_serve_disabled_is_noop() {
        let dash = Dashboard::disabled();
        let result = dash.serve().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = DashboardEvent::CostUpdate {
            task_id: "T-7".into(),
            cost_usd: 0.0015,
            tokens: 2048,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DashboardEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            DashboardEvent::CostUpdate {
                task_id,
                cost_usd,
                tokens,
            } => {
                assert_eq!(task_id, "T-7");
                assert!((cost_usd - 0.0015).abs() < f64::EPSILON);
                assert_eq!(tokens, 2048);
            }
            _ => panic!("Expected CostUpdate"),
        }
    }

    #[test]
    fn test_db_handle() {
        let db = test_db();
        let dash = Dashboard::new(
            DashboardConfig::default(),
            db.clone(),
        );
        // Verify the handle is the same Arc
        assert!(Arc::ptr_eq(dash.db(), &db));
    }
}
