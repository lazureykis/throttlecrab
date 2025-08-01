use std::sync::Arc;
use throttlecrab_server::metrics::{Metrics, Transport};

#[test]
fn test_advanced_metrics() {
    let metrics = Arc::new(Metrics::new());

    // Test top denied keys
    metrics.record_request_with_key(Transport::Http, 1000, false, "user:123");
    metrics.record_request_with_key(Transport::Http, 1000, false, "user:123");
    metrics.record_request_with_key(Transport::Http, 1000, false, "user:456");
    metrics.record_request_with_key(Transport::Http, 1000, true, "user:789");

    // Test requests per minute
    for _ in 0..10 {
        metrics.record_request_with_key(Transport::Http, 1000, true, "test");
    }

    let prometheus = metrics.export_prometheus();

    // Verify top denied keys are exported
    assert!(prometheus.contains("throttlecrab_top_denied_keys"));
    assert!(prometheus.contains("user:123"));

    // Verify denial rate
    assert!(prometheus.contains("throttlecrab_denial_rate_percent"));

    // Verify requests per minute
    assert!(prometheus.contains("throttlecrab_requests_per_minute"));

    // Extract the requests per minute value
    let rpm = metrics.get_requests_per_minute();
    assert_eq!(rpm, 14); // 4 (3 denied + 1 allowed from top denied test) + 10 allowed

    // Verify cleanup metrics
    assert!(prometheus.contains("throttlecrab_cleanup_duration_seconds"));
    assert!(prometheus.contains("throttlecrab_last_cleanup_evicted_keys"));

    // Verify estimated memory
    assert!(prometheus.contains("throttlecrab_estimated_memory_bytes"));
}

#[test]
fn test_denial_rate_calculation() {
    let metrics = Arc::new(Metrics::new());

    // 10 requests, 3 denied
    for i in 0..10 {
        let allowed = i >= 3;
        metrics.record_request_with_key(Transport::Http, 1000, allowed, "test");
    }

    let rate = metrics.get_denial_rate_percent();
    assert!((rate - 30.0).abs() < 0.01); // 30% denial rate

    // Test requests per minute
    let rpm = metrics.get_requests_per_minute();
    assert_eq!(rpm, 10);
}
