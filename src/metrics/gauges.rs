//! Gauge metrics

use std::sync::Arc;
use tokio::sync::RwLock;

/// A gauge metric that can be set to any value
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    description: String,
    value: Arc<RwLock<f64>>,
}

impl Gauge {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            value: Arc::new(RwLock::new(0.0)),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub async fn set(&self, value: f64) {
        let mut v = self.value.write().await;
        *v = value;
    }

    pub async fn get(&self) -> f64 {
        *self.value.read().await
    }

    pub async fn inc(&self, delta: f64) {
        let mut v = self.value.write().await;
        *v += delta;
    }

    pub async fn dec(&self, delta: f64) {
        let mut v = self.value.write().await;
        *v -= delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gauge() {
        let gauge = Gauge::new("active_sessions", "Active sessions");

        gauge.set(5.0).await;
        assert_eq!(gauge.get().await, 5.0);

        gauge.inc(3.0).await;
        assert_eq!(gauge.get().await, 8.0);

        gauge.dec(2.0).await;
        assert_eq!(gauge.get().await, 6.0);
    }
}
