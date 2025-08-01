//! Simple metrics collection for observability
//!
//! This module provides lightweight metrics collection using atomic counters.
//! Designed for minimal overhead and zero allocations in the hot path.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    /// Active connections by transport
    pub http_connections: AtomicUsize,
    pub grpc_connections: AtomicUsize,
    pub redis_connections: AtomicUsize,

    /// Request latency buckets (in microseconds)
    pub latency_under_1ms: AtomicU64,
    pub latency_under_10ms: AtomicU64,
    pub latency_under_100ms: AtomicU64,
    pub latency_under_1s: AtomicU64,
    pub latency_over_1s: AtomicU64,

    /// Histogram support
    pub latency_sum_micros: AtomicU64,

    /// Store metrics
    pub active_keys: AtomicUsize,
    pub store_evictions: AtomicU64,

    /// Advanced metrics
    pub(crate) top_denied_keys: Mutex<TopDeniedKeys>,
    pub denial_window_start: AtomicU64,
    pub requests_in_window: AtomicU64,
    pub denials_in_window: AtomicU64,
    pub requests_per_minute: Mutex<[AtomicU64; 60]>,
    pub current_minute: AtomicUsize,
    pub last_cleanup_duration_micros: AtomicU64,
    pub last_cleanup_evicted: AtomicU64,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Initialize requests_per_minute array
        let mut rpm_array = Vec::with_capacity(60);
        for _ in 0..60 {
            rpm_array.push(AtomicU64::new(0));
        }
        let rpm_array: [AtomicU64; 60] = rpm_array.try_into().unwrap();

        Self {
            start_time: Instant::now(),
            total_requests: AtomicU64::new(0),
            http_requests: AtomicU64::new(0),
            grpc_requests: AtomicU64::new(0),
            redis_requests: AtomicU64::new(0),
            requests_allowed: AtomicU64::new(0),
            requests_denied: AtomicU64::new(0),
            requests_errors: AtomicU64::new(0),
            http_connections: AtomicUsize::new(0),
            grpc_connections: AtomicUsize::new(0),
            redis_connections: AtomicUsize::new(0),
            latency_under_1ms: AtomicU64::new(0),
            latency_under_10ms: AtomicU64::new(0),
            latency_under_100ms: AtomicU64::new(0),
            latency_under_1s: AtomicU64::new(0),
            latency_over_1s: AtomicU64::new(0),
            latency_sum_micros: AtomicU64::new(0),
            active_keys: AtomicUsize::new(0),
            store_evictions: AtomicU64::new(0),
            top_denied_keys: Mutex::new(TopDeniedKeys::new(100)),
            denial_window_start: AtomicU64::new(now),
            requests_in_window: AtomicU64::new(0),
            denials_in_window: AtomicU64::new(0),
            requests_per_minute: Mutex::new(rpm_array),
            current_minute: AtomicUsize::new((now / 60) as usize),
            last_cleanup_duration_micros: AtomicU64::new(0),
            last_cleanup_evicted: AtomicU64::new(0),
        }
    }

    /// Record a request and its latency with key information
    pub fn record_request_with_key(
        &self,
        transport: Transport,
        latency_us: u64,
        allowed: bool,
        key: &str,
    ) {
        // Update all the metrics that don't need the key
        self.record_request(transport, latency_us, allowed);

        // Update advanced metrics
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Update denial window metrics
        let window_start = self.denial_window_start.load(Ordering::Relaxed);
        if now - window_start > 300 {
            // Reset 5-minute window
            self.denial_window_start.store(now, Ordering::Relaxed);
            self.requests_in_window.store(1, Ordering::Relaxed);
            self.denials_in_window
                .store(if allowed { 0 } else { 1 }, Ordering::Relaxed);
        } else {
            self.requests_in_window.fetch_add(1, Ordering::Relaxed);
            if !allowed {
                self.denials_in_window.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update requests per minute
        let current_minute = (now / 60) as usize;
        let last_minute = self.current_minute.load(Ordering::Relaxed);

        if current_minute != last_minute {
            // Clear old minutes if we've jumped ahead
            if let Ok(rpm) = self.requests_per_minute.lock() {
                let diff = current_minute.wrapping_sub(last_minute);
                if diff > 0 && diff < 60 {
                    for i in 1..=diff.min(60) {
                        let idx = (last_minute + i) % 60;
                        rpm[idx].store(0, Ordering::Relaxed);
                    }
                }
            }
            self.current_minute.store(current_minute, Ordering::Relaxed);
        }

        if let Ok(rpm) = self.requests_per_minute.lock() {
            let idx = current_minute % 60;
            rpm[idx].fetch_add(1, Ordering::Relaxed);
        }

        // Update top denied keys if request was denied
        if !allowed {
            if let Ok(mut top_keys) = self.top_denied_keys.lock() {
                top_keys.update(key.to_string());
            }
        }
    }

    /// Record a request and its latency
    pub fn record_request(&self, transport: Transport, latency_us: u64, allowed: bool) {
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
    }

    /// Update active connection count
    #[allow(dead_code)]
    pub fn connection_opened(&self, transport: Transport) {
        match transport {
            Transport::Http => self.http_connections.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_connections.fetch_add(1, Ordering::Relaxed),
            Transport::Redis => self.redis_connections.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Update active connection count
    #[allow(dead_code)]
    pub fn connection_closed(&self, transport: Transport) {
        match transport {
            Transport::Http => self.http_connections.fetch_sub(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_connections.fetch_sub(1, Ordering::Relaxed),
            Transport::Redis => self.redis_connections.fetch_sub(1, Ordering::Relaxed),
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
            Transport::Http => self.http_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Grpc => self.grpc_requests.fetch_add(1, Ordering::Relaxed),
            Transport::Redis => self.redis_requests.fetch_add(1, Ordering::Relaxed),
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
    }

    /// Record store cleanup metrics
    #[allow(dead_code)]
    pub fn record_cleanup(&self, duration: Duration, evicted_count: usize) {
        self.last_cleanup_duration_micros
            .store(duration.as_micros() as u64, Ordering::Relaxed);
        self.last_cleanup_evicted
            .store(evicted_count as u64, Ordering::Relaxed);
    }

    /// Get current requests per minute
    pub fn get_requests_per_minute(&self) -> u64 {
        if let Ok(rpm) = self.requests_per_minute.lock() {
            rpm.iter().map(|a| a.load(Ordering::Relaxed)).sum()
        } else {
            0
        }
    }

    /// Get denial rate percentage
    pub fn get_denial_rate_percent(&self) -> f64 {
        let requests = self.requests_in_window.load(Ordering::Relaxed);
        let denials = self.denials_in_window.load(Ordering::Relaxed);

        if requests == 0 {
            0.0
        } else {
            (denials as f64 / requests as f64) * 100.0
        }
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

        // Active connections
        output.push_str(
            "# HELP throttlecrab_connections_active Current active connections by transport\n",
        );
        output.push_str("# TYPE throttlecrab_connections_active gauge\n");
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"http\"}} {}\n",
            self.http_connections.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"grpc\"}} {}\n",
            self.grpc_connections.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "throttlecrab_connections_active{{transport=\"redis\"}} {}\n\n",
            self.redis_connections.load(Ordering::Relaxed)
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
        // Calculate total for +Inf bucket (sum of all buckets)
        let total_in_buckets = self.latency_under_1ms.load(Ordering::Relaxed)
            + self.latency_under_10ms.load(Ordering::Relaxed)
            + self.latency_under_100ms.load(Ordering::Relaxed)
            + self.latency_under_1s.load(Ordering::Relaxed)
            + self.latency_over_1s.load(Ordering::Relaxed);

        output.push_str(&format!(
            "throttlecrab_request_duration_bucket{{le=\"+Inf\"}} {total_in_buckets}\n"
        ));

        // Add sum and count for proper histogram
        let latency_sum_seconds =
            self.latency_sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        output.push_str(&format!(
            "throttlecrab_request_duration_sum {latency_sum_seconds:.6}\n"
        ));
        output.push_str(&format!(
            "throttlecrab_request_duration_count {total_in_buckets}\n\n"
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
            "throttlecrab_store_evictions {}\n\n",
            self.store_evictions.load(Ordering::Relaxed)
        ));

        // Advanced metrics

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
        output.push('\n');

        // Denial rate
        output.push_str("# HELP throttlecrab_denial_rate_percent Percentage of requests denied in last 5 minutes\n");
        output.push_str("# TYPE throttlecrab_denial_rate_percent gauge\n");
        output.push_str(&format!(
            "throttlecrab_denial_rate_percent {:.2}\n\n",
            self.get_denial_rate_percent()
        ));

        // Requests per minute
        output.push_str(
            "# HELP throttlecrab_requests_per_minute Total requests in the last minute\n",
        );
        output.push_str("# TYPE throttlecrab_requests_per_minute gauge\n");
        output.push_str(&format!(
            "throttlecrab_requests_per_minute {}\n\n",
            self.get_requests_per_minute()
        ));

        // Store performance metrics
        output.push_str(
            "# HELP throttlecrab_cleanup_duration_seconds Duration of last cleanup operation\n",
        );
        output.push_str("# TYPE throttlecrab_cleanup_duration_seconds gauge\n");
        let cleanup_duration_secs =
            self.last_cleanup_duration_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        output.push_str(&format!(
            "throttlecrab_cleanup_duration_seconds {cleanup_duration_secs:.6}\n\n"
        ));

        output.push_str("# HELP throttlecrab_last_cleanup_evicted_keys Number of keys evicted in last cleanup\n");
        output.push_str("# TYPE throttlecrab_last_cleanup_evicted_keys gauge\n");
        output.push_str(&format!(
            "throttlecrab_last_cleanup_evicted_keys {}\n\n",
            self.last_cleanup_evicted.load(Ordering::Relaxed)
        ));

        // Estimated memory usage (rough approximation: 100 bytes per key)
        let estimated_memory = self.active_keys.load(Ordering::Relaxed) * 100;
        output.push_str(
            "# HELP throttlecrab_estimated_memory_bytes Estimated memory usage in bytes\n",
        );
        output.push_str("# TYPE throttlecrab_estimated_memory_bytes gauge\n");
        output.push_str(&format!(
            "throttlecrab_estimated_memory_bytes {estimated_memory}\n"
        ));

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
        metrics.record_request(Transport::Http, 500, true);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.http_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.latency_under_1ms.load(Ordering::Relaxed), 1);

        // Record a denied gRPC request
        metrics.record_request(Transport::Grpc, 50000, false);

        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.grpc_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.http_requests.load(Ordering::Relaxed), 1);
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
        metrics.connection_opened(Transport::Http);

        let output = metrics.export_prometheus();

        // Check that output contains expected metrics
        assert!(output.contains("throttlecrab_uptime_seconds"));
        assert!(output.contains("throttlecrab_requests_total 2"));
        assert!(output.contains("throttlecrab_requests_allowed 1"));
        assert!(output.contains("throttlecrab_requests_denied 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"http\"} 1"));
        assert!(output.contains("throttlecrab_requests_by_transport{transport=\"grpc\"} 1"));
        assert!(output.contains("throttlecrab_connections_active{transport=\"http\"} 1"));
    }

    #[test]
    fn test_counter_consistency() {
        let metrics = Metrics::new();

        // Record various requests
        metrics.record_request(Transport::Http, 500, true); // allowed
        metrics.record_request(Transport::Http, 1500, false); // denied
        metrics.record_request(Transport::Grpc, 2500, true); // allowed
        metrics.record_request(Transport::Grpc, 50000, false); // denied
        metrics.record_error(Transport::Http, 10000); // error

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

        // Verify histogram buckets sum correctly
        let bucket_sum = metrics.latency_under_1ms.load(Ordering::Relaxed)
            + metrics.latency_under_10ms.load(Ordering::Relaxed)
            + metrics.latency_under_100ms.load(Ordering::Relaxed)
            + metrics.latency_under_1s.load(Ordering::Relaxed)
            + metrics.latency_over_1s.load(Ordering::Relaxed);
        assert_eq!(bucket_sum, 5);

        // Verify specific counts
        assert_eq!(metrics.requests_allowed.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.requests_denied.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.requests_errors.load(Ordering::Relaxed), 1);

        // Verify histogram export consistency
        let output = metrics.export_prometheus();
        assert!(output.contains("throttlecrab_request_duration_bucket{le=\"+Inf\"} 5"));
        assert!(output.contains("throttlecrab_request_duration_count 5"));
    }
}
