//! Metrics Module
//!
//! Provides Prometheus-compatible metrics for monitoring d3vx performance.
//!
//! # Usage
//!
//! ```rust
//! # tokio_test::block_on(async {
//! use d3vx::metrics::{inc_counter, observe_histogram, set_gauge};
//!
//! // Increment a counter
//! inc_counter("agent", "messages_sent", 1.0);
//!
//! // Record a histogram
//! observe_histogram("provider", "request_duration_seconds", 0.15);
//!
//! // Set a gauge
//! set_gauge("agent", "active_sessions", 3.0);
//! # });
//! ```

pub mod counters;
pub mod gauges;
pub mod histograms;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// A single metric data point
#[derive(Debug, Clone)]
pub struct MetricValue {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
}

/// Metrics registry for storing and retrieving metrics
pub struct MetricsRegistry {
    counters: Arc<RwLock<HashMap<String, f64>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, HistogramData>>>,
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Increment a counter
    pub async fn inc_counter(&self, name: &str, value: f64) {
        let mut counters = self.counters.write().await;
        let entry = counters.entry(name.to_string()).or_insert(0.0);
        *entry += value;
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    /// Record a histogram value
    pub async fn observe_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        let data = histograms
            .entry(name.to_string())
            .or_insert_with(|| HistogramData::new());
        data.observe(value);
    }

    /// Get all counter values
    pub async fn get_counters(&self) -> HashMap<String, f64> {
        self.counters.read().await.clone()
    }

    /// Get all gauge values
    pub async fn get_gauges(&self) -> HashMap<String, f64> {
        self.gauges.read().await.clone()
    }

    /// Get all histogram data
    pub async fn get_histograms(&self) -> HashMap<String, HistogramData> {
        self.histograms.read().await.clone()
    }

    /// Export all metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        let counters = self.counters.read().await;
        for (name, value) in counters.iter() {
            let safe_name = name.replace('.', "_").replace('-', "_");
            output.push_str(&format!("# TYPE {} counter\n", safe_name));
            output.push_str(&format!("{} {{}} {}\n", safe_name, value));
        }

        // Export gauges
        let gauges = self.gauges.read().await;
        for (name, value) in gauges.iter() {
            let safe_name = name.replace('.', "_").replace('-', "_");
            output.push_str(&format!("# TYPE {} gauge\n", safe_name));
            output.push_str(&format!("{} {{}} {}\n", safe_name, value));
        }

        // Export histograms
        let histograms = self.histograms.read().await;
        for (name, data) in histograms.iter() {
            let safe_name = name.replace('.', "_").replace('-', "_");
            output.push_str(&format!("# TYPE {} histogram\n", safe_name));
            output.push_str(&format!("{}_count {{}} {}\n", safe_name, data.count));
            output.push_str(&format!("{}_sum {{}} {}\n", safe_name, data.sum));
            output.push_str(&format!("{}_min {{}} {}\n", safe_name, data.min));
            output.push_str(&format!("{}_max {{}} {}\n", safe_name, data.max));
            output.push_str(&format!("{}_mean {{}} {}\n", safe_name, data.mean()));
        }

        output
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        self.counters.write().await.clear();
        self.gauges.write().await.clear();
        self.histograms.write().await.clear();
    }
}

/// Histogram data storage
#[derive(Debug, Clone)]
pub struct HistogramData {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub values: Vec<f64>,
}

impl HistogramData {
    pub fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            values: Vec::new(),
        }
    }

    pub fn observe(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.values.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Calculate percentiles (approximate)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }

        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx = ((p / 100.0) * sorted.len() as f64).floor() as usize;
        sorted
            .get(idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(0.0)
    }
}

impl Default for HistogramData {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────────
// Global Registry
// ────────────────────────────────────────────────────────────

lazy_static::lazy_static! {
    pub static ref METRICS: MetricsRegistry = MetricsRegistry::new();
}

// ────────────────────────────────────────────────────────────
// Convenience Functions
// ────────────────────────────────────────────────────────────

/// Increment a counter
pub fn inc_counter(subsystem: &str, name: &str, value: f64) {
    let full_name = format!("{}_{}", subsystem, name);
    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        METRICS.inc_counter(&full_name, value).await;
    });
}

/// Set a gauge value
pub fn set_gauge(subsystem: &str, name: &str, value: f64) {
    let full_name = format!("{}_{}", subsystem, name);
    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        METRICS.set_gauge(&full_name, value).await;
    });
}

/// Record a histogram observation
pub fn observe_histogram(subsystem: &str, name: &str, value: f64) {
    let full_name = format!("{}_{}", subsystem, name);
    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        METRICS.observe_histogram(&full_name, value).await;
    });
}

/// Time a block of code and record duration
#[macro_export]
macro_rules! time_operation {
    ($subsystem:expr, $name:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed().as_secs_f64();
        $crate::metrics::observe_histogram($subsystem, $name, duration);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_counter() {
        let registry = MetricsRegistry::new();

        registry.inc_counter("test_counter", 1.0).await;
        registry.inc_counter("test_counter", 2.0).await;

        let counters = registry.get_counters().await;
        assert_eq!(counters.get("test_counter"), Some(&3.0));
    }

    #[tokio::test]
    async fn test_gauge() {
        let registry = MetricsRegistry::new();

        registry.set_gauge("test_gauge", 10.0).await;
        registry.set_gauge("test_gauge", 5.0).await;

        let gauges = registry.get_gauges().await;
        assert_eq!(gauges.get("test_gauge"), Some(&5.0));
    }

    #[tokio::test]
    async fn test_histogram() {
        let registry = MetricsRegistry::new();

        registry.observe_histogram("test_hist", 1.0).await;
        registry.observe_histogram("test_hist", 2.0).await;
        registry.observe_histogram("test_hist", 3.0).await;

        let histograms = registry.get_histograms().await;
        let hist = histograms.get("test_hist").unwrap();

        assert_eq!(hist.count, 3);
        assert_eq!(hist.sum, 6.0);
        assert_eq!(hist.min, 1.0);
        assert_eq!(hist.max, 3.0);
        assert!((hist.mean() - 2.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_export() {
        let registry = MetricsRegistry::new();

        registry.inc_counter("test_counter", 5.0).await;
        registry.set_gauge("test_gauge", 10.0).await;

        let output = registry.export_prometheus().await;

        assert!(output.contains("test_counter"));
        assert!(output.contains("test_gauge"));
        assert!(output.contains("# TYPE"));
    }
}
