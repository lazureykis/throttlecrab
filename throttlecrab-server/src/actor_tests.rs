#[cfg(test)]
mod tests {
    use crate::actor::RateLimiterActor;
    use crate::types::ThrottleRequest;
    use throttlecrab::PeriodicStore;

    #[tokio::test]
    async fn test_basic_rate_limiting() {
        let store = PeriodicStore::builder()
            .capacity(1000)
            .cleanup_interval(std::time::Duration::from_secs(60))
            .build();
        let handle = RateLimiterActor::spawn_periodic(100, store);

        // First request should succeed
        let req = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 5,
            count_per_period: 10,
            period: 60,
            quantity: 1,
            timestamp: std::time::SystemTime::now(),
        };

        let resp = handle.throttle(req.clone()).await.unwrap();
        assert!(resp.allowed);
        assert_eq!(resp.limit, 5);
        assert_eq!(resp.remaining, 4);
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let store = PeriodicStore::builder()
            .capacity(1000)
            .cleanup_interval(std::time::Duration::from_secs(60))
            .build();
        let handle = RateLimiterActor::spawn_periodic(100, store);

        let req = ThrottleRequest {
            key: "concurrent_test".to_string(),
            max_burst: 10,
            count_per_period: 10,
            period: 60,
            quantity: 1,
            timestamp: std::time::SystemTime::now(),
        };

        // Send multiple concurrent requests
        let mut handles = vec![];
        for _ in 0..20 {
            let h = handle.clone();
            let r = req.clone();
            handles.push(tokio::spawn(async move { h.throttle(r).await }));
        }

        // Collect results
        let mut allowed_count = 0;
        for h in handles {
            let result = h.await.unwrap().unwrap();
            if result.allowed {
                allowed_count += 1;
            }
        }

        // Should allow exactly burst capacity
        assert_eq!(allowed_count, 10);
    }
}
