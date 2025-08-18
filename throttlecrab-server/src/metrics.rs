//! Simple metrics collection for observability
//!
//! This module provides lightweight metrics collection using atomic counters.
//! Designed for minimal overhead and zero allocations in the hot path.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Maximum length allowed for rate limit keys
const MAX_KEY_LENGTH: usize = 256;

/// Tracks top N denied keys using HashMap for counts
pub(crate) struct TopDeniedKeys {
    counts: HashMap<String, u64>,
    max_size: usize,
}

impl TopDeniedKeys {
    fn new(max_size: usize) -> Self {
        Self {
            counts: HashMap::with_capacity(max_size * 2),
            max_size,
        }
    }

    fn update(&mut self, key: String) {
        // Validate key length to prevent memory exhaustion
        if key.len() > MAX_KEY_LENGTH {
            return;
        }

        // Update count
        *self.counts.entry(key).or_insert(0) += 1;

        // Periodically clean up if we have too many entries
        if self.counts.len() > self.max_size * 3 {
            self.cleanup();
        }
    }

    fn cleanup(&mut self) {
        if self.counts.len() <= self.max_size {
            return;
        }

        // Get all entries and sort by count
        let mut entries: Vec<_> = self.counts.drain().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep only top max_size entries
        entries.truncate(self.max_size);
        self.counts = entries.into_iter().collect();
    }

    fn get_top(&self) -> Vec<(String, u64)> {
        let mut entries: Vec<_> = self.counts.iter().map(|(k, v)| (k.clone(), *v)).collect();

        // Sort by count descending
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Take only top N
        entries.truncate(self.max_size);
        entries
    }
}

/// Core metrics collected by the server
pub struct Metrics {
    /// Server start time
    start_time: Instant,

    /// Total requests received
    pub total_requests: AtomicU64,

    /// Requests by transport
    pub http_requests: AtomicU64,
    pub grpc_requests: AtomicU64,
    pub redis_requests: AtomicU64,

    /// Rate limiting decisions
    pub requests_allowed: AtomicU64,
    pub requests_denied: AtomicU64,
    pub requests_errors: AtomicU64,

    /// Top denied keys tracking
    pub(crate) top_denied_keys: Mutex<TopDeniedKeys>,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_requests: AtomicU64::new(0),
            http_requests: AtomicU64::new(0),
            grpc_requests: AtomicU64::new(0),
            redis_requests: AtomicU64::new(0),
            requests_allowed: AtomicU64::new(0),
            requests_denied: AtomicU64::new(0),
            requests_errors: AtomicU64::new(0),
            top_denied_keys: Mutex::new(TopDeniedKeys::new(100)),
        }
    }

    /// Record a request with key information
    pub fn record_request_with_key(&self, transport: Transport, allowed: bool, key: &str) {
        // Update all the metrics that don't need the key
        self.record_request(transport, allowed);

        // Update top denied keys if request was denied
        if !allowed && let Ok(mut top_keys) = self.top_denied_keys.lock() {
            top_keys.update(key.to_string());
        }
    }

    /// Record a request
    pub fn record_request(&self, transport: Transport, allowed: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Record transport-specific counter
        match transport {
            Transport::Http => self.http_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Redis => self.redis_requests.fetch_add(1, Ordering::Relaxed),
        };

        // Record allow/deny decision
        if allowed {
            self.requests_allowed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_denied.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record an internal error
    pub fn record_error(&self, transport: Transport) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.requests_errors.fetch_add(1, Ordering::Relaxed);

        // Record transport-specific counter
        match transport {
            Transport::Http => self.http_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Redis => self.redis_requests.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Get server uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Escape a string for use as a Prometheus label value
    fn escape_prometheus_label(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 2);
        for ch in s.chars() {
            match ch {
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                // Control characters
                c if c.is_control() => {
                    result.push_str(&format!("\\x{:02x}", c as u8));
                }
                c => result.push(c),
            }
        }
        result
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        // Estimate size: ~50 chars per metric line, ~7 metrics = ~350 chars
        let mut output = String::with_capacity(500);

        // Add header
        output.push_str("# HELP throttlecrab_uptime_seconds Time since server start in seconds\n");
        output.push_str("# TYPE throttlecrab_uptime_seconds gauge\n");
        output.push_str(&format!(
            "throttlecrab_uptime_seconds {}\n\n",
            self.uptime_seconds()
        ));

        // Total requests
        output.push_str("# HELP throttlecrab_requests_total Total number of requests processed\n");
        output.push_str("# TYPE throttlecrab_requests_total counter\n");
        output.push_str(&format!(
            "throttlecrab_requests_total {}\n\n",
            self.total_requests.load(Ordering::Relaxed)
        ));

        // Requests by transport
        output.push_str(
            "# HELP throttlecrab_requests_by_transport Total requests by transport type\n",
        );
        output.push_str("# TYPE throttlecrab_requests_by_transport counter\n");
        output.push_str(&format!(
            "throttlecrab_requests_by_transport{{transport=\"http\"}} {}\n",
            self.http_requests.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_requests_by_transport{{transport=\"grpc\"}} {}\n",
            self.grpc_requests.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_requests_by_transport{{transport=\"redis\"}} {}\n\n",
            self.redis_requests.load(Ordering::Relaxed)
        ));

        // Allow/Deny decisions
        output.push_str("# HELP throttlecrab_requests_allowed Total requests allowed\n");
        output.push_str("# TYPE throttlecrab_requests_allowed counter\n");
        output.push_str(&format!(
            "throttlecrab_requests_allowed {}\n\n",
            self.requests_allowed.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP throttlecrab_requests_denied Total requests denied\n");
        output.push_str("# TYPE throttlecrab_requests_denied counter\n");
        output.push_str(&format!(
            "throttlecrab_requests_denied {}\n\n",
            self.requests_denied.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP throttlecrab_requests_errors Total internal errors\n");
        output.push_str("# TYPE throttlecrab_requests_errors counter\n");
        output.push_str(&format!(
            "throttlecrab_requests_errors {}\n\n",
            self.requests_errors.load(Ordering::Relaxed)
        ));

        // Top denied keys
        output.push_str("# HELP throttlecrab_top_denied_keys Top keys by denial count\n");
        output.push_str("# TYPE throttlecrab_top_denied_keys gauge\n");
        if let Ok(top_keys) = self.top_denied_keys.lock() {
            for (rank, (key, count)) in top_keys.get_top().iter().enumerate() {
                output.push_str(&format!(
                    "throttlecrab_top_denied_keys{{key=\"{}\",rank=\"{}\"}} {}\n",
                    Self::escape_prometheus_label(key),
                    rank + 1,
                    count
                ));
            }
        }

        output
    }
}

/// Transport type for metrics tracking
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    Http,
    Grpc,
    Redis,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.requests_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_request() {
        let metrics = Metrics::new();

        // Record an allowed HTTP request
        metrics.record_request(Transport::Http, true);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.http_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 0);

        // Record a denied gRPC request
        metrics.record_request(Transport::Grpc, false);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.grpc_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.http_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = Metrics::new();

        // Add some test data
        metrics.record_request(Transport::Http, true);
        metrics.record_request(Transport::Grpc, false);

        let output = metrics.export_prometheus();

        // Check that output contains expected metrics
        assert!(output.contains("throttlecrab_uptime_seconds"));
        assert!(output.contains("throttlecrab_requests_total 2"));
        assert!(output.contains("throttlecrab_requests_allowed 1"));
        assert!(output.contains("throttlecrab_requests_denied 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"http\"} 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"grpc\"} 1"));
    }

    #[test]
    fn test_counter_consistency() {
        let metrics = Metrics::new();

        // Record various requests
        metrics.record_request(Transport::Http, true); // allowed
        metrics.record_request(Transport::Http, false); // denied
        metrics.record_request(Transport::Grpc, true); // allowed
        metrics.record_request(Transport::Grpc, false); // denied
        metrics.record_error(Transport::Http); // error

        // Verify total requests
        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 5);

        // Verify transport counters sum to total
        let transport_sum = metrics.http_requests.load(Ordering::Relaxed)
            + metrics.grpc_requests.load(Ordering::Relaxed);
        assert_eq!(transport_sum, 5);

        // Verify allowed + denied + errors = total
        let decision_sum = metrics.requests_allowed.load(Ordering::Relaxed)
            + metrics.requests_denied.load(Ordering::Relaxed)
            + metrics.requests_errors.load(Ordering::Relaxed);
        assert_eq!(decision_sum, 5);

        // Verify specific counts
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.requests_errors.load(Ordering::Relaxed), 1);
    }
}
