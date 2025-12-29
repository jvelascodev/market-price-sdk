//! Provider health metrics collection and reporting
//!
//! Tracks latency histograms and success rates for price providers.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Maximum number of samples to keep for metrics calculation
const MAX_SAMPLES: usize = 100;

/// Metrics for a single provider
#[derive(Debug, Clone)]
pub struct ProviderMetrics {
    /// Name of the provider
    pub provider_name: String,
    /// 50th percentile latency in milliseconds
    pub latency_p50_ms: f64,
    /// 99th percentile latency in milliseconds
    pub latency_p99_ms: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total number of requests tracked
    pub total_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
}

impl ProviderMetrics {
    /// Creates metrics with no data
    pub fn empty(provider_name: &str) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            latency_p50_ms: 0.0,
            latency_p99_ms: 0.0,
            success_rate: 1.0,
            total_requests: 0,
            failed_requests: 0,
        }
    }
}

/// Internal sample for latency tracking
#[derive(Debug, Clone)]
struct LatencySample {
    duration_ms: f64,
    success: bool,
}

/// Collects and computes metrics for providers
pub struct MetricsCollector {
    /// Provider name
    provider_name: String,
    /// Rolling window of latency samples
    samples: Arc<RwLock<VecDeque<LatencySample>>>,
    /// Total requests (lifetime)
    total_requests: Arc<RwLock<u64>>,
    /// Failed requests (lifetime)
    failed_requests: Arc<RwLock<u64>>,
}

impl MetricsCollector {
    /// Creates a new metrics collector for a provider
    pub fn new(provider_name: &str) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            samples: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_SAMPLES))),
            total_requests: Arc::new(RwLock::new(0)),
            failed_requests: Arc::new(RwLock::new(0)),
        }
    }

    /// Records a request with its duration and success status
    pub async fn record_request(&self, duration: Duration, success: bool) {
        let duration_ms = duration.as_secs_f64() * 1000.0;

        // Update totals
        {
            let mut total = self.total_requests.write().await;
            *total += 1;
        }

        if !success {
            let mut failed = self.failed_requests.write().await;
            *failed += 1;
        }

        // Add sample to rolling window
        {
            let mut samples = self.samples.write().await;
            if samples.len() >= MAX_SAMPLES {
                samples.pop_front();
            }
            samples.push_back(LatencySample {
                duration_ms,
                success,
            });
        }
    }

    /// Computes current metrics from collected samples
    pub async fn get_metrics(&self) -> ProviderMetrics {
        let samples = self.samples.read().await;
        let total = *self.total_requests.read().await;
        let failed = *self.failed_requests.read().await;

        if samples.is_empty() {
            return ProviderMetrics::empty(&self.provider_name);
        }

        // Extract successful latencies for percentile calculation
        let mut latencies: Vec<f64> = samples
            .iter()
            .filter(|s| s.success)
            .map(|s| s.duration_ms)
            .collect();

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p50 = percentile(&latencies, 50.0);
        let p99 = percentile(&latencies, 99.0);

        let success_rate = if total > 0 {
            (total - failed) as f64 / total as f64
        } else {
            1.0
        };

        ProviderMetrics {
            provider_name: self.provider_name.clone(),
            latency_p50_ms: p50,
            latency_p99_ms: p99,
            success_rate,
            total_requests: total,
            failed_requests: failed,
        }
    }
}

/// Calculate percentile from sorted values
fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let idx = (p / 100.0 * (sorted_values.len() - 1) as f64).round() as usize;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

/// RAII guard for timing requests
pub struct RequestTimer {
    start: Instant,
    collector: Arc<MetricsCollector>,
    success: bool,
}

impl RequestTimer {
    /// Creates a new request timer
    pub fn new(collector: Arc<MetricsCollector>) -> Self {
        Self {
            start: Instant::now(),
            collector,
            success: false,
        }
    }

    /// Marks the request as successful
    pub fn mark_success(&mut self) {
        self.success = true;
    }
}

impl Drop for RequestTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        let collector = self.collector.clone();
        let success = self.success;
        
        // Spawn a task to record the metric asynchronously
        tokio::spawn(async move {
            collector.record_request(duration, success).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new("test");

        // Record some requests
        collector.record_request(Duration::from_millis(100), true).await;
        collector.record_request(Duration::from_millis(200), true).await;
        collector.record_request(Duration::from_millis(150), false).await;

        let metrics = collector.get_metrics().await;

        assert_eq!(metrics.provider_name, "test");
        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.failed_requests, 1);
        assert!(metrics.success_rate > 0.6 && metrics.success_rate < 0.7);
    }

    #[test]
    fn test_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(percentile(&values, 50.0), 5.0);
        assert_eq!(percentile(&values, 99.0), 10.0);
    }
}
