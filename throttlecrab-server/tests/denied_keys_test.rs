use std::sync::Arc;
use throttlecrab_server::metrics::{Metrics, Transport};

#[test]
fn test_configurable_denied_keys_tracking() {
    // Test with default (100 keys)
    let metrics_default = Arc::new(Metrics::new());

    // Add 150 different denied keys
    for i in 0..150 {
        metrics_default.record_request_with_key(Transport::Http, false, &format!("user:{}", i));
    }

    let prometheus = metrics_default.export_prometheus();
    let denied_keys_count = prometheus.matches("throttlecrab_top_denied_keys{").count();

    // Should track up to 100 keys (default)
    assert!(denied_keys_count <= 100);

    // Test with custom limit (200 keys)
    let metrics_custom = Arc::new(Metrics::builder().max_denied_keys(200).build());

    // Add 250 different denied keys
    for i in 0..250 {
        metrics_custom.record_request_with_key(Transport::Http, false, &format!("user:{}", i));
    }

    let prometheus = metrics_custom.export_prometheus();
    let denied_keys_count = prometheus.matches("throttlecrab_top_denied_keys{").count();

    // Should track up to 200 keys (custom limit)
    assert!(denied_keys_count <= 200);
    assert!(denied_keys_count > 100); // Should be more than default
}

#[test]
fn test_denied_keys_with_multiple_denials() {
    let metrics = Arc::new(Metrics::builder().max_denied_keys(10).build());

    // Add keys with different denial counts
    for _ in 0..10 {
        metrics.record_request_with_key(Transport::Http, false, "top_key");
    }
    for _ in 0..5 {
        metrics.record_request_with_key(Transport::Http, false, "medium_key");
    }
    for _ in 0..2 {
        metrics.record_request_with_key(Transport::Http, false, "low_key");
    }

    // Add many single-denial keys
    for i in 0..20 {
        metrics.record_request_with_key(Transport::Http, false, &format!("single_{}", i));
    }

    let prometheus = metrics.export_prometheus();

    // Should contain the most denied keys
    assert!(prometheus.contains("top_key"));
    assert!(prometheus.contains("medium_key"));
    assert!(prometheus.contains("low_key"));

    // Check ranking
    assert!(prometheus.contains("rank=\"1\""));
    assert!(prometheus.contains("rank=\"2\""));
    assert!(prometheus.contains("rank=\"3\""));
}

#[test]
fn test_max_denied_keys_limit() {
    // Test that the limit is capped at 10,000 even if we request more
    let metrics = Arc::new(Metrics::builder()
            .max_denied_keys(20_000) // Request more than the limit
            .build());

    // Add 15,000 different denied keys
    for i in 0..15_000 {
        metrics.record_request_with_key(Transport::Http, false, &format!("user:{}", i));
    }

    let prometheus = metrics.export_prometheus();
    let denied_keys_count = prometheus.matches("throttlecrab_top_denied_keys{").count();

    // Should be capped at 10,000 (the maximum allowed)
    assert!(denied_keys_count <= 10_000);
}

#[test]
fn test_disabled_denied_keys_tracking() {
    // Test that setting max_denied_keys to 0 disables tracking entirely
    let metrics = Arc::new(Metrics::builder()
            .max_denied_keys(0) // Disable tracking
            .build());

    // Record many denied requests
    for i in 0..100 {
        metrics.record_request_with_key(Transport::Http, false, &format!("user:{}", i));
    }

    let prometheus = metrics.export_prometheus();

    // Should NOT contain any denied keys metrics
    assert!(!prometheus.contains("throttlecrab_top_denied_keys"));

    // But should still contain regular metrics
    assert!(prometheus.contains("throttlecrab_requests_total"));
    assert!(prometheus.contains("throttlecrab_requests_denied 100"));
}
