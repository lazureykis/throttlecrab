use std::time::Duration;
use throttlecrab_client::{ClientBuilder, ThrottleCrabClient};
use throttlecrab_server::actor::RateLimiterActor;
use throttlecrab_server::transport::{Transport, native::NativeTransport};
use tokio::time::sleep;

#[tokio::test]
async fn test_basic_rate_limiting() {
    // Start server
    let store = throttlecrab::PeriodicStore::builder()
        .capacity(1000)
        .cleanup_interval(Duration::from_secs(60))
        .build();
    let limiter = RateLimiterActor::spawn_periodic(1000, store);

    // Get actual port
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let actual_port = listener.local_addr().unwrap().port();
    drop(listener);

    let transport = NativeTransport::new("127.0.0.1", actual_port);

    // Spawn server in background
    tokio::spawn(async move {
        transport.start(limiter).await.unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    // Create client
    let client = ThrottleCrabClient::connect(format!("127.0.0.1:{actual_port}"))
        .await
        .unwrap();

    // Test rate limiting
    let response = client
        .check_rate_limit("test_key", 10, 100, 60)
        .await
        .unwrap();

    assert!(response.allowed);
    assert_eq!(response.limit, 10);
    assert_eq!(response.remaining, 9);
}

#[tokio::test]
async fn test_connection_pooling() {
    // Start server
    let store = throttlecrab::PeriodicStore::builder()
        .capacity(1000)
        .cleanup_interval(Duration::from_secs(60))
        .build();
    let limiter = RateLimiterActor::spawn_periodic(1000, store);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let actual_port = listener.local_addr().unwrap().port();
    drop(listener);

    let transport = NativeTransport::new("127.0.0.1", actual_port);

    // Spawn server in background
    tokio::spawn(async move {
        transport.start(limiter).await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    // Create client with custom pool config
    let client = ClientBuilder::new()
        .max_connections(5)
        .min_idle_connections(2)
        .connect_timeout(Duration::from_secs(2))
        .request_timeout(Duration::from_secs(5))
        .build(format!("127.0.0.1:{actual_port}"))
        .await
        .unwrap();

    // Pool should have 2 connections after warm-up
    assert!(client.pool_size() >= 2);

    // Make multiple concurrent requests
    let mut handles = vec![];
    for i in 0..10 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client
                .check_rate_limit(format!("test_key_{i}"), 100, 1000, 60)
                .await
        }));
    }

    // Wait for all requests
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert!(result.unwrap().allowed);
    }
}

#[tokio::test]
async fn test_rate_limit_exceeded() {
    // Start server
    let store = throttlecrab::PeriodicStore::builder()
        .capacity(1000)
        .cleanup_interval(Duration::from_secs(60))
        .build();
    let limiter = RateLimiterActor::spawn_periodic(1000, store);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let actual_port = listener.local_addr().unwrap().port();
    drop(listener);

    let transport = NativeTransport::new("127.0.0.1", actual_port);

    tokio::spawn(async move {
        transport.start(limiter).await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    let client = ThrottleCrabClient::connect(format!("127.0.0.1:{actual_port}"))
        .await
        .unwrap();

    // Use unique key to avoid conflicts with other tests
    let key = format!(
        "rate_limit_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let max_burst = 2;
    let count_per_period = 10; // Allow 10 per period, but burst is only 2
    let period = 10; // 10 seconds

    // First two requests should succeed
    for i in 0..2 {
        let response = client
            .check_rate_limit(&key, max_burst, count_per_period, period)
            .await
            .unwrap();

        assert!(response.allowed, "Request {} should be allowed", i + 1);
        assert_eq!(response.remaining, (max_burst - (i as i64) - 1));
    }

    // Third request should be rate limited
    let response = client
        .check_rate_limit(&key, max_burst, count_per_period, period)
        .await
        .unwrap();
    assert!(!response.allowed, "Third request should be rate limited");
    assert_eq!(response.remaining, 0);
    assert!(response.retry_after > 0);
}
