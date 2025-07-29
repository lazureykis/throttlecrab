use super::{RateLimiterActor, ThrottleRequest};
use std::time::Duration;
use tokio::time::{Instant, sleep};

#[tokio::test]
async fn test_basic_rate_limiting() {
    let handle = RateLimiterActor::spawn(100);

    // First request should succeed
    let req = ThrottleRequest {
        key: "test".to_string(),
        max_burst: 5,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    let resp = handle.throttle(req.clone()).await.unwrap();
    assert!(resp.allowed);
    assert_eq!(resp.limit, 5);
    assert_eq!(resp.remaining, 4);
}

#[tokio::test]
async fn test_burst_capacity() {
    let handle = RateLimiterActor::spawn(100);

    let req = ThrottleRequest {
        key: "burst_test".to_string(),
        max_burst: 5,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    // Should allow burst capacity requests
    for i in 0..5 {
        let resp = handle.throttle(req.clone()).await.unwrap();
        assert!(resp.allowed, "Request {} should be allowed", i + 1);
        // After using i+1 tokens, we should have burst - (i+1) remaining
        assert_eq!(resp.remaining, 5 - (i + 1) as i64);
    }

    // 6th request should be blocked
    let resp = handle.throttle(req.clone()).await.unwrap();
    assert!(!resp.allowed);
    assert_eq!(resp.remaining, 0);
    assert!(resp.retry_after > 0);
}

#[tokio::test]
async fn test_multiple_keys() {
    let handle = RateLimiterActor::spawn(100);

    let req1 = ThrottleRequest {
        key: "user1".to_string(),
        max_burst: 3,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    let req2 = ThrottleRequest {
        key: "user2".to_string(),
        max_burst: 3,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    // Both users should have independent limits
    let resp1 = handle.throttle(req1.clone()).await.unwrap();
    let resp2 = handle.throttle(req2.clone()).await.unwrap();

    assert!(resp1.allowed);
    assert!(resp2.allowed);
    assert_eq!(resp1.remaining, 2);
    assert_eq!(resp2.remaining, 2);

    // Exhaust user1's limit
    for _ in 0..2 {
        handle.throttle(req1.clone()).await.unwrap();
    }

    let resp1 = handle.throttle(req1.clone()).await.unwrap();
    assert!(!resp1.allowed);

    // User2 should still be allowed
    let resp2 = handle.throttle(req2.clone()).await.unwrap();
    assert!(resp2.allowed);
}

#[tokio::test]
async fn test_quantity_parameter() {
    let handle = RateLimiterActor::spawn(100);

    let req = ThrottleRequest {
        key: "quantity_test".to_string(),
        max_burst: 10,
        count_per_period: 20,
        period: 60,
        quantity: 5, // Request 5 tokens at once
    };

    let resp = handle.throttle(req.clone()).await.unwrap();
    assert!(resp.allowed);
    assert_eq!(resp.remaining, 5); // 10 - 5 = 5

    // Request another 5
    let resp = handle.throttle(req.clone()).await.unwrap();
    assert!(resp.allowed);
    assert_eq!(resp.remaining, 0);

    // Next request should fail
    let resp = handle.throttle(req).await.unwrap();
    assert!(!resp.allowed);
}

#[tokio::test]
async fn test_rate_replenishment() {
    let handle = RateLimiterActor::spawn(100);

    let req = ThrottleRequest {
        key: "replenish_test".to_string(),
        max_burst: 2,
        count_per_period: 60, // 1 per second
        period: 60,
        quantity: 1,
    };

    // Use up burst
    for _ in 0..2 {
        let resp = handle.throttle(req.clone()).await.unwrap();
        assert!(resp.allowed);
    }

    // Should be blocked
    let resp = handle.throttle(req.clone()).await.unwrap();
    assert!(!resp.allowed);

    // Wait for token replenishment
    sleep(Duration::from_secs(2)).await;

    // Should be allowed again
    let resp = handle.throttle(req).await.unwrap();
    assert!(resp.allowed);
}

#[tokio::test]
async fn test_invalid_parameters() {
    let handle = RateLimiterActor::spawn(100);

    // Negative burst
    let req = ThrottleRequest {
        key: "invalid".to_string(),
        max_burst: -1,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    let result = handle.throttle(req).await;
    assert!(result.is_err());

    // Zero period
    let req = ThrottleRequest {
        key: "invalid".to_string(),
        max_burst: 10,
        count_per_period: 10,
        period: 0,
        quantity: 1,
    };

    let result = handle.throttle(req).await;
    assert!(result.is_err());

    // Negative quantity
    let req = ThrottleRequest {
        key: "invalid".to_string(),
        max_burst: 10,
        count_per_period: 10,
        period: 60,
        quantity: -1,
    };

    let result = handle.throttle(req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_requests() {
    let handle = RateLimiterActor::spawn(1000);

    let req = ThrottleRequest {
        key: "concurrent".to_string(),
        max_burst: 100,
        count_per_period: 100,
        period: 60,
        quantity: 1,
    };

    // Spawn 100 concurrent requests
    let mut handles = vec![];
    for _ in 0..100 {
        let h = handle.clone();
        let r = req.clone();
        handles.push(tokio::spawn(async move { h.throttle(r).await }));
    }

    // Collect results
    let mut allowed_count = 0;
    for h in handles {
        if let Ok(Ok(resp)) = h.await {
            if resp.allowed {
                allowed_count += 1;
            }
        }
    }

    // All 100 should be allowed due to burst capacity
    assert_eq!(allowed_count, 100);
}

#[tokio::test]
async fn test_reset_after_timing() {
    let handle = RateLimiterActor::spawn(100);

    let req = ThrottleRequest {
        key: "reset_test".to_string(),
        max_burst: 5,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    let _start = Instant::now();
    let resp = handle.throttle(req).await.unwrap();

    // reset_after is when the bucket fully resets
    // With burst=5, emission=6s, tolerance=24s
    // After first request, TAT=now, so reset_after = tolerance = 24s
    assert!(resp.reset_after >= 20 && resp.reset_after <= 30);
}

#[tokio::test]
async fn test_different_rate_limits_same_key() {
    let handle = RateLimiterActor::spawn(100);

    // First request with one rate limit
    let req1 = ThrottleRequest {
        key: "flex_test".to_string(),
        max_burst: 5,
        count_per_period: 10,
        period: 60,
        quantity: 1,
    };

    let resp = handle.throttle(req1).await.unwrap();
    assert!(resp.allowed);
    assert_eq!(resp.limit, 5);

    // Same key but different rate limit parameters
    // This tests that rate limits are determined by request, not stored per key
    let req2 = ThrottleRequest {
        key: "flex_test".to_string(),
        max_burst: 10,
        count_per_period: 20,
        period: 60,
        quantity: 1,
    };

    let resp = handle.throttle(req2).await.unwrap();
    // The result depends on the TAT stored, but with different parameters
    assert_eq!(resp.limit, 10);
}

#[tokio::test]
async fn test_high_burst_low_rate() {
    let handle = RateLimiterActor::spawn(100);

    let req = ThrottleRequest {
        key: "high_burst".to_string(),
        max_burst: 1000,
        count_per_period: 10,
        period: 3600, // Very low rate: 10 per hour
        quantity: 1,
    };

    // Should allow full burst
    for i in 0..1000 {
        let resp = handle.throttle(req.clone()).await.unwrap();
        assert!(resp.allowed, "Request {} should be allowed", i + 1);
    }

    // 1001st request should be blocked
    let resp = handle.throttle(req).await.unwrap();
    assert!(!resp.allowed);
    assert_eq!(resp.remaining, 0);
}
