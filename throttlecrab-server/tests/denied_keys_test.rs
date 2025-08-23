use std::sync::Arc;
use throttlecrab_server::metrics::{Metrics, Transport};

#[test]
fn test_configurable_denied_keys_tracking() {
    // Test with default (100 keys)
    let metrics_default = Arc::new(Metrics::new());
    
    // Add 150 different denied keys
    for i in 0..150 {
        metrics_default.record_request_with_key(
            Transport::Http,
            false,
            &format!("user:{}", i),
        );
    }
    
    let prometheus = metrics_default.export_prometheus();
    let denied_keys_count = prometheus.matches("throttlecrab_top_denied_keys{").count();
    
    // Should track up to 100 keys (default)
    assert!(denied_keys_count <= 100);
    
    // Test with custom limit (200 keys)
    let metrics_custom = Arc::new(
        Metrics::builder()
            .max_denied_keys(200)
            .build()
    );
    
    // Add 250 different denied keys
    for i in 0..250 {
        metrics_custom.record_request_with_key(
            Transport::Http,
            false,
            &format!("user:{}", i),
        );
    }
    
    let prometheus = metrics_custom.export_prometheus();
    let denied_keys_count = prometheus.matches("throttlecrab_top_denied_keys{").count();
    
    // Should track up to 200 keys (custom limit)
    assert!(denied_keys_count <= 200);
    assert!(denied_keys_count > 100); // Should be more than default
}

#[test]
fn test_denied_keys_with_multiple_denials() {
    let metrics = Arc::new(
        Metrics::builder()
            .max_denied_keys(10)
            .build()
    );
    
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
        metrics.record_request_with_key(
            Transport::Http,
            false,
            &format!("single_{}", i),
        );
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