use std::sync::Arc;
use throttlecrab_server::metrics::{Metrics, Transport};

#[test]
fn test_top_denied_keys() {
    let metrics = Arc::new(Metrics::new());

    // Test top denied keys
    metrics.record_request_with_key(Transport::Http, false, "user:123");
    metrics.record_request_with_key(Transport::Http, false, "user:123");
    metrics.record_request_with_key(Transport::Http, false, "user:456");
    metrics.record_request_with_key(Transport::Http, true, "user:789");

    let prometheus = metrics.export_prometheus();

    // Verify top denied keys are exported
    assert!(prometheus.contains("throttlecrab_top_denied_keys"));
    assert!(prometheus.contains("user:123"));
    assert!(prometheus.contains("user:456"));
    // user:789 should not appear as it was allowed
    assert!(!prometheus.contains("user:789"));
}

#[test]
fn test_request_counting() {
    let metrics = Arc::new(Metrics::new());

    // 10 requests, 3 denied
    for i in 0..10 {
        let allowed = i >= 3;
        metrics.record_request_with_key(Transport::Http, allowed, "test");
    }

    let prometheus = metrics.export_prometheus();

    // Verify request counts
    assert!(prometheus.contains("throttlecrab_requests_total 10"));
    assert!(prometheus.contains("throttlecrab_requests_allowed 7"));
    assert!(prometheus.contains("throttlecrab_requests_denied 3"));
}
