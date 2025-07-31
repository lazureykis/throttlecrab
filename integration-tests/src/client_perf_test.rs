//! Performance test for throttlecrab native client

use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use throttlecrab_client::ClientBuilder;
use tokio::sync::Barrier;
use tokio::task::JoinSet;

#[derive(Debug)]
pub struct ClientPerfStats {
    pub total_requests: AtomicU64,
    pub successful: AtomicU64,
    pub rate_limited: AtomicU64,
    pub failed: AtomicU64,
    pub total_latency_us: AtomicU64,
}

impl ClientPerfStats {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
        }
    }
}

pub async fn run_client_performance_test(
    num_threads: usize,
    requests_per_thread: usize,
    port: u16,
    pool_size: usize,
) -> Result<()> {
    println!("=== Native Client Performance Test ===");
    println!("Threads: {num_threads}");
    println!("Requests per thread: {requests_per_thread}");
    let total_expected = num_threads * requests_per_thread;
    println!("Total requests: {total_expected}");
    println!("Connection pool size: {pool_size}");
    println!("Target port: {port}\n");

    // Create shared client with connection pool
    let client = ClientBuilder::new()
        .max_connections(pool_size)
        .min_idle_connections(pool_size / 2)
        .connect_timeout(Duration::from_secs(5))
        .request_timeout(Duration::from_secs(30))
        .tcp_nodelay(true)
        .build(format!("127.0.0.1:{port}"))
        .await?;

    println!(
        "Client connected. Initial pool size: {}",
        client.pool_size()
    );

    // Create shared resources
    let stats = Arc::new(ClientPerfStats::new());
    let barrier = Arc::new(Barrier::new(num_threads + 1));
    let start_flag = Arc::new(AtomicU64::new(0));

    // Spawn worker threads
    let mut tasks = JoinSet::new();
    for thread_id in 0..num_threads {
        let client = client.clone();
        let stats = stats.clone();
        let barrier = barrier.clone();
        let start_flag = start_flag.clone();

        tasks.spawn(async move {
            // Wait for all threads to be ready
            barrier.wait().await;

            // Wait for start signal
            while start_flag.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
            }

            // Send requests
            for i in 0..requests_per_thread {
                let key = format!("thread_{thread_id}_request_{i}");
                let start = Instant::now();

                match client.check_rate_limit(&key, 10000, 100000, 60).await {
                    Ok(response) => {
                        let latency = start.elapsed();
                        stats.total_requests.fetch_add(1, Ordering::Relaxed);
                        stats
                            .total_latency_us
                            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);

                        if response.allowed {
                            stats.successful.fetch_add(1, Ordering::Relaxed);
                        } else {
                            stats.rate_limited.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        stats.failed.fetch_add(1, Ordering::Relaxed);
                        stats.total_requests.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    // Wait for all threads to be ready
    println!("Waiting for all threads to be ready...");
    barrier.wait().await;

    // Start the benchmark
    println!("Starting benchmark...");
    let bench_start = Instant::now();
    start_flag.store(1, Ordering::Release);

    // Wait for all tasks to complete
    while let Some(result) = tasks.join_next().await {
        result?;
    }

    let duration = bench_start.elapsed();

    // Calculate and print results
    let total = stats.total_requests.load(Ordering::Relaxed);
    let successful = stats.successful.load(Ordering::Relaxed);
    let rate_limited = stats.rate_limited.load(Ordering::Relaxed);
    let failed = stats.failed.load(Ordering::Relaxed);
    let total_latency_us = stats.total_latency_us.load(Ordering::Relaxed);

    let rps = total as f64 / duration.as_secs_f64();
    let avg_latency_us = if total > 0 {
        total_latency_us / total
    } else {
        0
    };

    println!("\n=== Benchmark Results ===");
    println!("Duration: {duration:?}");
    println!("Total requests: {total}");
    println!("Throughput: {rps:.2} requests/sec");
    let successful_pct = successful as f64 / total as f64 * 100.0;
    println!("Successful: {successful} ({successful_pct:.2}%)");
    let rate_limited_pct = rate_limited as f64 / total as f64 * 100.0;
    println!("Rate limited: {rate_limited} ({rate_limited_pct:.2}%)");
    let failed_pct = failed as f64 / total as f64 * 100.0;
    println!("Failed: {failed} ({failed_pct:.2}%)");
    println!("Average latency: {avg_latency_us} μs");

    println!("\nFinal pool status:");
    let pool_size = client.pool_size();
    println!("Pool size: {pool_size}");
    let available = client.available_connections();
    println!("Available connections: {available}");

    Ok(())
}
