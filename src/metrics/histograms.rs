//! Histogram metrics

use std::sync::Arc;
use tokio::sync::RwLock;

/// A histogram metric that tracks distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    description: String,
    data: Arc<RwLock<HistogramInner>>,
}

#[derive(Debug, Clone)]
pub struct HistogramInner {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl Histogram {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            data: Arc::new(RwLock::new(HistogramInner {
                count: 0,
                sum: 0.0,
                min: f64::MAX,
                max: f64::MIN,
            })),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub async fn observe(&self, value: f64) {
        let mut data = self.data.write().await;
        data.count += 1;
        data.sum += value;
        data.min = data.min.min(value);
        data.max = data.max.max(value);
    }

    pub async fn stats(&self) -> HistogramStats {
        let data = self.data.read().await;
        HistogramStats {
            count: data.count,
            sum: data.sum,
            min: if data.count > 0 { data.min } else { 0.0 },
            max: if data.count > 0 { data.max } else { 0.0 },
            mean: if data.count > 0 {
                data.sum / data.count as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistogramStats {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_histogram() {
        let hist = Histogram::new("request_duration", "Request duration");

        hist.observe(0.1).await;
        hist.observe(0.2).await;
        hist.observe(0.3).await;

        let stats = hist.stats().await;
        assert_eq!(stats.count, 3);
        assert!((stats.mean - 0.2).abs() < 0.001);
    }
}
