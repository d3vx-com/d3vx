//! Counter metrics

use std::sync::Arc;
use tokio::sync::RwLock;

/// A counter metric that can only be incremented
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    description: String,
    value: Arc<RwLock<f64>>,
}

impl Counter {
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

    pub async fn inc(&self, value: f64) {
        let mut v = self.value.write().await;
        *v += value;
    }

    pub async fn get(&self) -> f64 {
        *self.value.read().await
    }

    pub async fn reset(&self) {
        let mut v = self.value.write().await;
        *v = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_counter() {
        let counter = Counter::new("requests", "Total requests");

        counter.inc(1.0).await;
        counter.inc(5.0).await;

        assert_eq!(counter.get().await, 6.0);
    }
}
