use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio::task::JoinSet;

#[derive(Debug)]
pub struct Stats {
    pub total_requests: AtomicU64,
    pub successful: AtomicU64,
    pub rate_limited: AtomicU64,
    pub failed: AtomicU64,
    pub total_latency_us: AtomicU64,
}

impl Stats {
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

#[derive(Clone)]
pub enum Transport {
    Http,
    Grpc,
    Redis,
}

impl Transport {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "http" => Ok(Transport::Http),
            "grpc" => Ok(Transport::Grpc),
            "redis" => Ok(Transport::Redis),
            _ => anyhow::bail!("Invalid transport: {}. Valid options: http, grpc", s),
        }
    }
}

async fn http_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    stats: Arc<Stats>,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<Vec<Duration>> {
    // Create HTTP client with connection pooling
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(30))
        .build()?;

    let url = format!("http://127.0.0.1:{port}/throttle");

    // Pre-generate payloads
    let mut payloads = Vec::with_capacity(requests_per_thread);
    for i in 0..requests_per_thread {
        let key = format!("key_{thread_id}_req_{i}");
        payloads.push(json!({
            "key": key,
            "max_burst": 100,
            "count_per_period": 10,
            "period": 60,
            "quantity": 1,
        }));
    }

    // Wait for all threads to be ready
    barrier.wait().await;

    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }

    let mut latencies = Vec::with_capacity(requests_per_thread);

    // Send all requests
    for payload in payloads {
        let start = Instant::now();

        match client.post(&url).json(&payload).send().await {
            Ok(response) => {
                let latency = start.elapsed();
                latencies.push(latency);
                stats.total_requests.fetch_add(1, Ordering::Relaxed);
                stats
                    .total_latency_us
                    .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);

                if let Ok(body) = response.json::<serde_json::Value>().await {
                    if body["allowed"].as_bool().unwrap_or(true) {
                        stats.successful.fetch_add(1, Ordering::Relaxed);
                    } else {
                        stats.rate_limited.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    stats.failed.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(_) => {
                stats.failed.fetch_add(1, Ordering::Relaxed);
                stats.total_requests.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    Ok(latencies)
}

async fn grpc_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    stats: Arc<Stats>,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<Vec<Duration>> {
    use throttlecrab_server::grpc::ThrottleRequest;
    use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;

    // Create gRPC client
    let mut client = RateLimiterClient::connect(format!("http://127.0.0.1:{port}")).await?;

    // Pre-generate requests
    let mut requests = Vec::with_capacity(requests_per_thread);
    for i in 0..requests_per_thread {
        let key = format!("key_{}_{}", thread_id, i % 1000);
        requests.push(ThrottleRequest {
            key,
            max_burst: 100,
            count_per_period: 10,
            period: 60,
            quantity: 1,
        });
    }

    // Wait for all threads to be ready
    barrier.wait().await;

    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }

    let mut latencies = Vec::with_capacity(requests_per_thread);

    // Send all requests
    for request in requests {
        let start = Instant::now();

        match client.throttle(tonic::Request::new(request)).await {
            Ok(response) => {
                let latency = start.elapsed();
                latencies.push(latency);
                stats.total_requests.fetch_add(1, Ordering::Relaxed);
                stats
                    .total_latency_us
                    .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);

                if response.into_inner().allowed {
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

    Ok(latencies)
}

async fn redis_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    stats: Arc<Stats>,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<Vec<Duration>> {
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    // Connect to Redis server
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
    
    // Pre-generate THROTTLE commands
    let mut commands = Vec::with_capacity(requests_per_thread);
    for i in 0..requests_per_thread {
        let key = format!("key_{}_{}", thread_id, i % 1000);
        // THROTTLE key max_burst count_per_period period quantity
        let cmd = format!(
            "*6\r\n$8\r\nTHROTTLE\r\n${}\r\n{}\r\n$3\r\n100\r\n$2\r\n10\r\n$2\r\n60\r\n$1\r\n1\r\n",
            key.len(), key
        );
        commands.push(cmd.into_bytes());
    }
    
    // Wait for all threads to be ready
    barrier.wait().await;
    
    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }
    
    let mut latencies = Vec::with_capacity(requests_per_thread);
    let mut read_buf = vec![0u8; 1024];
    
    // Send all requests
    for command in commands {
        let start = Instant::now();
        
        match stream.write_all(&command).await {
            Ok(_) => {
                // Read response (we expect an array response)
                match stream.read(&mut read_buf).await {
                    Ok(n) if n > 0 => {
                        let latency = start.elapsed();
                        latencies.push(latency);
                        stats.total_requests.fetch_add(1, Ordering::Relaxed);
                        stats
                            .total_latency_us
                            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
                        
                        // Simple check if allowed (response starts with "*5\r\n:1")
                        if n > 7 && &read_buf[0..7] == b"*5\r\n:1" {
                            stats.successful.fetch_add(1, Ordering::Relaxed);
                        } else {
                            stats.rate_limited.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    _ => {
                        stats.failed.fetch_add(1, Ordering::Relaxed);
                        stats.total_requests.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Err(_) => {
                stats.failed.fetch_add(1, Ordering::Relaxed);
                stats.total_requests.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    
    // Send QUIT command
    stream.write_all(b"*1\r\n$4\r\nQUIT\r\n").await?;
    
    Ok(latencies)
}

pub async fn run_performance_test(
    num_threads: usize,
    requests_per_thread: usize,
    port: u16,
    transport_str: &str,
) -> Result<()> {
    let transport = Transport::from_str(transport_str)?;

    println!("=== ThrottleCrab Performance Test ===");
    println!("Transport: {transport_str}");
    println!("Threads: {num_threads}");
    println!("Requests per thread: {requests_per_thread}");
    println!("Total requests: {}", num_threads * requests_per_thread);
    println!("Target port: {port}\n");

    // Check if server is running (quick HTTP health check)
    println!("Checking if server is running on port {port}...");
    match transport {
        Transport::Http => {
            let test_client = reqwest::Client::new();
            let test_url = format!("http://127.0.0.1:{port}/throttle");

            match test_client
                .post(&test_url)
                .json(&json!({
                    "key": "test",
                    "max_burst": 1,
                    "count_per_period": 1,
                    "period": 1,
                    "quantity": 1,
                }))
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                Ok(_) => println!("Server is running!"),
                Err(e) => {
                    eprintln!("Server is not running on port {port}: {e}");
                    eprintln!("Please start the server with the appropriate transport enabled");
                    return Err(anyhow::anyhow!("Server not running"));
                }
            }
        }
        _ => {
            // For non-HTTP transports, we'll assume the server is running
            println!("Assuming server is running with {transport_str} transport");
        }
    }

    // Create shared resources
    let stats = Arc::new(Stats::new());
    let barrier = Arc::new(Barrier::new(num_threads + 1)); // +1 for main thread
    let start_flag = Arc::new(AtomicU64::new(0));

    // Spawn worker threads
    let mut tasks = JoinSet::new();
    for thread_id in 0..num_threads {
        let stats = stats.clone();
        let barrier = barrier.clone();
        let start_flag = start_flag.clone();
        let transport = transport.clone();

        tasks.spawn(async move {
            match transport {
                Transport::Http => {
                    http_worker(
                        thread_id,
                        requests_per_thread,
                        port,
                        stats,
                        barrier,
                        start_flag,
                    )
                    .await
                }
                Transport::Grpc => {
                    grpc_worker(
                        thread_id,
                        requests_per_thread,
                        port,
                        stats,
                        barrier,
                        start_flag,
                    )
                    .await
                }
                Transport::Redis => {
                    redis_worker(
                        thread_id,
                        requests_per_thread,
                        port,
                        stats,
                        barrier,
                        start_flag,
                    )
                    .await
                }
            }
        });
    }

    // Wait for all threads to be ready
    println!("\nWaiting for all threads to establish connections...");
    barrier.wait().await;

    // Start the benchmark
    println!("Starting benchmark...");
    let bench_start = Instant::now();
    start_flag.store(1, Ordering::Release);

    // Collect all latencies
    let mut all_latencies = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(latencies)) => all_latencies.extend(latencies),
            Ok(Err(e)) => eprintln!("Worker error: {e}"),
            Err(e) => eprintln!("Task error: {e}"),
        }
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
    println!(
        "Successful: {} ({:.2}%)",
        successful,
        successful as f64 / total as f64 * 100.0
    );
    println!(
        "Rate limited: {} ({:.2}%)",
        rate_limited,
        rate_limited as f64 / total as f64 * 100.0
    );
    println!(
        "Failed: {} ({:.2}%)",
        failed,
        failed as f64 / total as f64 * 100.0
    );
    println!("Average latency: {avg_latency_us} μs");

    // Calculate percentiles
    if !all_latencies.is_empty() {
        all_latencies.sort();
        let p50 = percentile(&all_latencies, 0.5);
        let p90 = percentile(&all_latencies, 0.9);
        let p95 = percentile(&all_latencies, 0.95);
        let p99 = percentile(&all_latencies, 0.99);
        let p999 = percentile(&all_latencies, 0.999);

        println!("\nLatency percentiles:");
        println!("  P50: {p50:?}");
        println!("  P90: {p90:?}");
        println!("  P95: {p95:?}");
        println!("  P99: {p99:?}");
        println!("  P99.9: {p999:?}");
    }

    Ok(())
}

fn percentile(sorted_values: &[Duration], p: f64) -> Duration {
    if sorted_values.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted_values.len() as f64 - 1.0) * p) as usize;
    sorted_values[index]
}
