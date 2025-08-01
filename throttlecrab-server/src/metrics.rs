//! Simple metrics collection for observability
//!
//! This module provides lightweight metrics collection using atomic counters.
//! Designed for minimal overhead and zero allocations in the hot path.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// Core metrics collected by the server
pub struct Metrics {
    /// Server start time
    start_time: Instant,

    /// Total requests received
    pub total_requests: AtomicU64,

    /// Requests by transport
    pub native_requests: AtomicU64,
    pub http_requests: AtomicU64,
    pub grpc_requests: AtomicU64,

    /// Rate limiting decisions
    pub requests_allowed: AtomicU64,
    pub requests_denied: AtomicU64,
    pub requests_errors: AtomicU64,

    /// Active connections by transport
    pub native_connections: AtomicUsize,
    pub http_connections: AtomicUsize,
    pub grpc_connections: AtomicUsize,

    /// Request latency buckets (in microseconds)
    pub latency_under_1ms: AtomicU64,
    pub latency_under_10ms: AtomicU64,
    pub latency_under_100ms: AtomicU64,
    pub latency_under_1s: AtomicU64,
    pub latency_over_1s: AtomicU64,

    /// Histogram support
    pub latency_sum_micros: AtomicU64,
    pub latency_count: AtomicU64,

    /// Store metrics
    pub active_keys: AtomicUsize,
    pub store_evictions: AtomicU64,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_requests: AtomicU64::new(0),
            native_requests: AtomicU64::new(0),
            http_requests: AtomicU64::new(0),
            grpc_requests: AtomicU64::new(0),
            requests_allowed: AtomicU64::new(0),
            requests_denied: AtomicU64::new(0),
            requests_errors: AtomicU64::new(0),
            native_connections: AtomicUsize::new(0),
            http_connections: AtomicUsize::new(0),
            grpc_connections: AtomicUsize::new(0),
            latency_under_1ms: AtomicU64::new(0),
            latency_under_10ms: AtomicU64::new(0),
            latency_under_100ms: AtomicU64::new(0),
            latency_under_1s: AtomicU64::new(0),
            latency_over_1s: AtomicU64::new(0),
            latency_sum_micros: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
            active_keys: AtomicUsize::new(0),
            store_evictions: AtomicU64::new(0),
        }
    }

    /// Record a request and its latency
    pub fn record_request(&self, transport: Transport, latency_us: u64, allowed: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Record transport-specific counter
        match transport {
            Transport::Native => self.native_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Http => self.http_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_requests.fetch_add(1, Ordering::Relaxed),
        };

        // Record allow/deny decision
        if allowed {
            self.requests_allowed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_denied.fetch_add(1, Ordering::Relaxed);
        }

        // Record latency bucket
        match latency_us {
            0..=999 => self.latency_under_1ms.fetch_add(1, Ordering::Relaxed),
            1000..=9999 => self.latency_under_10ms.fetch_add(1, Ordering::Relaxed),
            10000..=99999 => self.latency_under_100ms.fetch_add(1, Ordering::Relaxed),
            100000..=999999 => self.latency_under_1s.fetch_add(1, Ordering::Relaxed),
            _ => self.latency_over_1s.fetch_add(1, Ordering::Relaxed),
        };

        // Update histogram metrics
        self.latency_sum_micros
            .fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update active connection count
    pub fn connection_opened(&self, transport: Transport) {
        match transport {
            Transport::Native => self.native_connections.fetch_add(1, Ordering::Relaxed),
            Transport::Http => self.http_connections.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_connections.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Update active connection count
    pub fn connection_closed(&self, transport: Transport) {
        match transport {
            Transport::Native => self.native_connections.fetch_sub(1, Ordering::Relaxed),
            Transport::Http => self.http_connections.fetch_sub(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_connections.fetch_sub(1, Ordering::Relaxed),
        };
    }

    /// Update active keys count
    #[allow(dead_code)]
    pub fn update_active_keys(&self, count: usize) {
        self.active_keys.store(count, Ordering::Relaxed);
    }

    /// Record a store eviction
    #[allow(dead_code)]
    pub fn record_eviction(&self) {
        self.store_evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an internal error
    pub fn record_error(&self, transport: Transport, latency_us: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.requests_errors.fetch_add(1, Ordering::Relaxed);

        // Record transport-specific counter
        match transport {
            Transport::Native => self.native_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Http => self.http_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_requests.fetch_add(1, Ordering::Relaxed),
        };

        // Record latency bucket even for errors
        match latency_us {
            0..=999 => self.latency_under_1ms.fetch_add(1, Ordering::Relaxed),
            1000..=9999 => self.latency_under_10ms.fetch_add(1, Ordering::Relaxed),
            10000..=99999 => self.latency_under_100ms.fetch_add(1, Ordering::Relaxed),
            100000..=999999 => self.latency_under_1s.fetch_add(1, Ordering::Relaxed),
            _ => self.latency_over_1s.fetch_add(1, Ordering::Relaxed),
        };

        // Update histogram metrics for errors too
        self.latency_sum_micros
            .fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get server uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        // Estimate size: ~50 chars per metric line, ~30 metrics = ~1500 chars
        let mut output = String::with_capacity(1500);

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
            "throttlecrab_requests_by_transport{{transport=\"native\"}} {}\n",
            self.native_requests.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_requests_by_transport{{transport=\"http\"}} {}\n",
            self.http_requests.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_requests_by_transport{{transport=\"grpc\"}} {}\n\n",
            self.grpc_requests.load(Ordering::Relaxed)
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

        // Active connections
        output.push_str(
            "# HELP throttlecrab_connections_active Current active connections by transport\n",
        );
        output.push_str("# TYPE throttlecrab_connections_active gauge\n");
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"native\"}} {}\n",
            self.native_connections.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"http\"}} {}\n",
            self.http_connections.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"grpc\"}} {}\n\n",
            self.grpc_connections.load(Ordering::Relaxed)
        ));

        // Latency distribution
        output
            .push_str("# HELP throttlecrab_request_duration_bucket Request latency distribution\n");
        output.push_str("# TYPE throttlecrab_request_duration_bucket histogram\n");
        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"0.001\"}} {}\n",
            self.latency_under_1ms.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"0.01\"}} {}\n",
            self.latency_under_1ms.load(Ordering::Relaxed)
                + self.latency_under_10ms.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"0.1\"}} {}\n",
            self.latency_under_1ms.load(Ordering::Relaxed)
                + self.latency_under_10ms.load(Ordering::Relaxed)
                + self.latency_under_100ms.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"1\"}} {}\n",
            self.latency_under_1ms.load(Ordering::Relaxed)
                + self.latency_under_10ms.load(Ordering::Relaxed)
                + self.latency_under_100ms.load(Ordering::Relaxed)
                + self.latency_under_1s.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"+Inf\"}} {}\n",
            self.total_requests.load(Ordering::Relaxed)
        ));

        // Add sum and count for proper histogram
        let latency_sum_seconds =
            self.latency_sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        output.push_str(&format!(
            "throttlecrab_request_duration_sum {latency_sum_seconds:.6}\n"
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_count {}\n\n",
            self.latency_count.load(Ordering::Relaxed)
        ));

        // Store metrics
        output.push_str("# HELP throttlecrab_active_keys Number of active rate limit keys\n");
        output.push_str("# TYPE throttlecrab_active_keys gauge\n");
        output.push_str(&format!(
            "throttlecrab_active_keys {}\n\n",
            self.active_keys.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP throttlecrab_store_evictions Total number of key evictions\n");
        output.push_str("# TYPE throttlecrab_store_evictions counter\n");
        output.push_str(&format!(
            "throttlecrab_store_evictions {}\n",
            self.store_evictions.load(Ordering::Relaxed)
        ));

        output
    }
}

/// Transport type for metrics tracking
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    Native,
    Http,
    Grpc,
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
        metrics.record_request(Transport::Http, 500, true);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.http_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.latency_under_1ms.load(Ordering::Relaxed), 1);

        // Record a denied Native request
        metrics.record_request(Transport::Native, 50000, false);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.native_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.latency_under_100ms.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_connection_tracking() {
        let metrics = Metrics::new();

        // Open connections
        metrics.connection_opened(Transport::Http);
        metrics.connection_opened(Transport::Http);
        metrics.connection_opened(Transport::Grpc);

        assert_eq!(metrics.http_connections.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.grpc_connections.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.native_connections.load(Ordering::Relaxed), 0);

        // Close one HTTP connection
        metrics.connection_closed(Transport::Http);

        assert_eq!(metrics.http_connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_latency_buckets() {
        let metrics = Metrics::new();

        // Test different latency ranges
        metrics.record_request(Transport::Http, 500, true); // < 1ms
        metrics.record_request(Transport::Http, 5000, true); // < 10ms
        metrics.record_request(Transport::Http, 50000, true); // < 100ms
        metrics.record_request(Transport::Http, 500000, true); // < 1s
        metrics.record_request(Transport::Http, 5000000, true); // > 1s

        assert_eq!(metrics.latency_under_1ms.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.latency_under_10ms.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.latency_under_100ms.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.latency_under_1s.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.latency_over_1s.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = Metrics::new();

        // Add some test data
        metrics.record_request(Transport::Http, 500, true);
        metrics.record_request(Transport::Grpc, 1500, false);
        metrics.connection_opened(Transport::Native);

        let output = metrics.export_prometheus();

        // Check that output contains expected metrics
        assert!(output.contains("throttlecrab_uptime_seconds"));
        assert!(output.contains("throttlecrab_requests_total 2"));
        assert!(output.contains("throttlecrab_requests_allowed 1"));
        assert!(output.contains("throttlecrab_requests_denied 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"http\"} 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"grpc\"} 1"));
        assert!(output.contains("throttlecrab_connections_active{transport=\"native\"} 1"));
    }
}
