use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use throttlecrab_client::ClientBuilder;
use throttlecrab_server::actor::RateLimiterActor;
use throttlecrab_server::transport::{Transport, native::NativeTransport};
use tokio::time::sleep;

#[derive(Default)]
struct PerfStats {
    total_requests: AtomicU64,
    successful: AtomicU64,
    rate_limited: AtomicU64,
    errors: AtomicU64,
    total_latency_us: AtomicU64,
}

impl PerfStats {
    fn record_success(&self, latency: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    fn record_rate_limited(&self, latency: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    fn record_error(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn print_summary(&self, duration: Duration) {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful.load(Ordering::Relaxed);
        let rate_limited = self.rate_limited.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);

        let rps = total as f64 / duration.as_secs_f64();
        let avg_latency = if total > 0 { total_latency / total } else { 0 };

        println!("\n=== Performance Test Results ===");
        println!("Duration: {duration:?}");
        println!("Total requests: {total}");
        println!("Throughput: {rps:.2} req/sec");
        let successful_pct = (successful as f64 / total as f64) * 100.0;
        println!("Successful: {successful} ({successful_pct:.2}%)");
        let rate_limited_pct = (rate_limited as f64 / total as f64) * 100.0;
        println!("Rate limited: {rate_limited} ({rate_limited_pct:.2}%)");
        let errors_pct = (errors as f64 / total as f64) * 100.0;
        println!("Errors: {errors} ({errors_pct:.2}%)");
        println!("Average latency: {avg_latency} μs");
    }
}

async fn setup_server() -> u16 {
    // Start server with large capacity
    let store = throttlecrab::PeriodicStore::builder()
        .capacity(1_000_000)
        .cleanup_interval(Duration::from_secs(300))
        .build();
    let limiter = RateLimiterActor::spawn_periodic(1_000_000, store);

    // Get random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let transport = NativeTransport::new("127.0.0.1", port);

    // Spawn server
    tokio::spawn(async move {
        transport.start(limiter).await.unwrap();
    });

    // Wait for server to start
    sleep(Duration::from_millis(100)).await;

    port
}

#[tokio::test(flavor = "multi_thread")]
async fn test_client_throughput() {
    let port = setup_server().await;

    // Create client with optimal settings
    let client = ClientBuilder::new()
        .max_idle_connections(20)
        .connect_timeout(Duration::from_secs(5))
        .request_timeout(Duration::from_secs(30))
        .tcp_nodelay(true)
        .build(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    let stats = Arc::new(PerfStats::default());
    let test_duration = Duration::from_secs(5);
    let start_time = Instant::now();

    // Spawn multiple workers
    let num_workers = 10;
    let requests_per_worker = 10_000;
    let mut handles = vec![];

    for worker_id in 0..num_workers {
        let client = client.clone();
        let stats = stats.clone();
        let handle = tokio::spawn(async move {
            for i in 0..requests_per_worker {
                let key = format!("worker_{worker_id}_key_{i}");
                let start = Instant::now();

                match client.check_rate_limit(&key, 1000, 10000, 60).await {
                    Ok(response) => {
                        let latency = start.elapsed();
                        if response.allowed {
                            stats.record_success(latency);
                        } else {
                            stats.record_rate_limited(latency);
                        }
                    }
                    Err(_) => {
                        stats.record_error();
                    }
                }

                // Stop if we've exceeded test duration
                if start_time.elapsed() > test_duration {
                    break;
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all workers
    for handle in handles {
        handle.await.unwrap();
    }

    let actual_duration = start_time.elapsed();
    stats.print_summary(actual_duration);

    // Assertions
    let total_requests = stats.total_requests.load(Ordering::Relaxed);
    let error_rate = stats.errors.load(Ordering::Relaxed) as f64 / total_requests as f64;

    assert!(total_requests > 10_000, "Should process many requests");
    assert!(error_rate < 0.01, "Error rate should be less than 1%");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pool_configurations() {
    let port = setup_server().await;
    let test_duration = Duration::from_secs(2);

    // Test different pool configurations
    let configurations = vec![
        ("Small pool", 2, 1),
        ("Medium pool", 10, 5),
        ("Large pool", 50, 20),
    ];

    for (name, max_conn, _min_idle) in configurations {
        println!("\n=== Testing {name} ===");

        let client = ClientBuilder::new()
            .max_idle_connections(max_conn)
            .tcp_nodelay(true)
            .build(format!("127.0.0.1:{port}"))
            .await
            .unwrap();

        let stats = Arc::new(PerfStats::default());
        let start_time = Instant::now();

        // Run concurrent requests
        let mut handles = vec![];
        for i in 0..20 {
            let client = client.clone();
            let stats = stats.clone();
            handles.push(tokio::spawn(async move {
                let mut count = 0;
                while start_time.elapsed() < test_duration {
                    let key = format!("pool_test_{i}_{count}");
                    let start = Instant::now();

                    match client.check_rate_limit(&key, 1000, 10000, 60).await {
                        Ok(response) => {
                            let latency = start.elapsed();
                            if response.allowed {
                                stats.record_success(latency);
                            } else {
                                stats.record_rate_limited(latency);
                            }
                        }
                        Err(_) => {
                            stats.record_error();
                        }
                    }
                    count += 1;
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        stats.print_summary(start_time.elapsed());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_connection_recovery() {
    let port = setup_server().await;

    let client = ClientBuilder::new()
        .max_idle_connections(5)
        .connect_timeout(Duration::from_secs(2))
        .request_timeout(Duration::from_secs(5))
        .build(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    let stats = Arc::new(PerfStats::default());

    // First, make some successful requests
    println!("Making initial requests...");
    for i in 0..100 {
        let start = Instant::now();
        match client
            .check_rate_limit(&format!("recovery_test_{i}"), 1000, 10000, 60)
            .await
        {
            Ok(response) => {
                let latency = start.elapsed();
                if response.allowed {
                    stats.record_success(latency);
                }
            }
            Err(_) => stats.record_error(),
        }
    }

    let initial_errors = stats.errors.load(Ordering::Relaxed);
    println!("Initial errors: {initial_errors}");

    // Simulate some connection issues by making many concurrent requests
    println!("\nStressing the connection pool...");
    let mut handles = vec![];
    for i in 0..50 {
        let client = client.clone();
        let stats = stats.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..20 {
                let start = Instant::now();
                match client
                    .check_rate_limit(&format!("stress_test_{i}_{j}"), 1000, 10000, 60)
                    .await
                {
                    Ok(response) => {
                        let latency = start.elapsed();
                        if response.allowed {
                            stats.record_success(latency);
                        }
                    }
                    Err(_) => stats.record_error(),
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Make more requests to verify recovery
    println!("\nTesting recovery...");
    let errors_before_recovery = stats.errors.load(Ordering::Relaxed);

    for i in 0..100 {
        let start = Instant::now();
        match client
            .check_rate_limit(&format!("recovery_test_2_{i}"), 1000, 10000, 60)
            .await
        {
            Ok(response) => {
                let latency = start.elapsed();
                if response.allowed {
                    stats.record_success(latency);
                }
            }
            Err(_) => stats.record_error(),
        }
    }

    let final_errors = stats.errors.load(Ordering::Relaxed);
    let recovery_errors = final_errors - errors_before_recovery;

    println!("Errors during recovery phase: {recovery_errors}");
    assert!(
        recovery_errors < 10,
        "Should have minimal errors after recovery"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_latency_percentiles() {
    let port = setup_server().await;

    let client = ClientBuilder::new()
        .max_idle_connections(10)
        .tcp_nodelay(true)
        .build(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    let mut latencies = Vec::new();
    let num_requests = 10_000;

    println!("Collecting latency data for {num_requests} requests...");

    for i in 0..num_requests {
        let key = format!("latency_test_{i}");
        let start = Instant::now();

        if let Ok(_response) = client.check_rate_limit(&key, 1000, 10000, 60).await {
            let latency = start.elapsed();
            latencies.push(latency);
        }

        // Add some variety in request timing
        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }

    // Calculate percentiles
    latencies.sort();
    let p50 = latencies[latencies.len() * 50 / 100];
    let p90 = latencies[latencies.len() * 90 / 100];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    println!("\n=== Latency Percentiles ===");
    println!("P50: {p50:?}");
    println!("P90: {p90:?}");
    println!("P95: {p95:?}");
    println!("P99: {p99:?}");

    // Assertions on latency
    assert!(p50 < Duration::from_millis(5), "P50 should be under 5ms");
    assert!(p99 < Duration::from_millis(50), "P99 should be under 50ms");
}
