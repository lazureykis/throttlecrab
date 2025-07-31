use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Barrier;
use tokio::task::JoinSet;

#[derive(Debug, Default)]
pub struct ThreadLocalStats {
    pub total_requests: u64,
    pub successful: u64,
    pub rate_limited: u64,
    pub failed: u64,
    pub total_latency_us: u64,
}

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

    pub fn add_thread_stats(&self, thread_stats: &ThreadLocalStats) {
        self.total_requests
            .fetch_add(thread_stats.total_requests, Ordering::Relaxed);
        self.successful
            .fetch_add(thread_stats.successful, Ordering::Relaxed);
        self.rate_limited
            .fetch_add(thread_stats.rate_limited, Ordering::Relaxed);
        self.failed
            .fetch_add(thread_stats.failed, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(thread_stats.total_latency_us, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub enum Transport {
    Http,
    Grpc,
    MsgPack,
    Native,
}

impl Transport {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "http" => Ok(Transport::Http),
            "grpc" => Ok(Transport::Grpc),
            "msgpack" => Ok(Transport::MsgPack),
            "native" => Ok(Transport::Native),
            _ => anyhow::bail!(
                "Invalid transport: {}. Valid options: http, grpc, msgpack, native",
                s
            ),
        }
    }
}

async fn http_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<(Vec<Duration>, ThreadLocalStats)> {
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
    let mut thread_stats = ThreadLocalStats::default();

    // Send all requests
    for payload in payloads {
        let start = Instant::now();

        match client.post(&url).json(&payload).send().await {
            Ok(response) => {
                let latency = start.elapsed();
                latencies.push(latency);
                thread_stats.total_requests += 1;
                thread_stats.total_latency_us += latency.as_micros() as u64;

                if let Ok(body) = response.json::<serde_json::Value>().await {
                    if body["allowed"].as_bool().unwrap_or(true) {
                        thread_stats.successful += 1;
                    } else {
                        thread_stats.rate_limited += 1;
                    }
                } else {
                    thread_stats.failed += 1;
                }
            }
            Err(_) => {
                thread_stats.failed += 1;
                thread_stats.total_requests += 1;
            }
        }
    }

    Ok((latencies, thread_stats))
}

async fn grpc_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<(Vec<Duration>, ThreadLocalStats)> {
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
            timestamp: 0,
        });
    }

    // Wait for all threads to be ready
    barrier.wait().await;

    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }

    let mut latencies = Vec::with_capacity(requests_per_thread);
    let mut thread_stats = ThreadLocalStats::default();

    // Send all requests
    for request in requests {
        let start = Instant::now();

        match client.throttle(tonic::Request::new(request)).await {
            Ok(response) => {
                let latency = start.elapsed();
                latencies.push(latency);
                thread_stats.total_requests += 1;
                thread_stats.total_latency_us += latency.as_micros() as u64;

                if response.into_inner().allowed {
                    thread_stats.successful += 1;
                } else {
                    thread_stats.rate_limited += 1;
                }
            }
            Err(_) => {
                thread_stats.failed += 1;
                thread_stats.total_requests += 1;
            }
        }
    }

    Ok((latencies, thread_stats))
}

async fn msgpack_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<(Vec<Duration>, ThreadLocalStats)> {
    use rmp_serde::{Deserializer, Serializer};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    struct Request {
        cmd: u8,
        key: String,
        burst: i64,
        rate: i64,
        period: i64,
        quantity: i64,
        timestamp: i64,
    }

    #[derive(Deserialize)]
    struct Response {
        ok: bool,
        allowed: u8,
        #[allow(dead_code)]
        limit: i64,
        #[allow(dead_code)]
        remaining: i64,
        #[allow(dead_code)]
        retry_after: i64,
        #[allow(dead_code)]
        reset_after: i64,
    }

    // Create a dedicated connection for this worker (no sharing between threads)
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).await?;
    stream.set_nodelay(true)?;

    // Pre-generate requests
    let mut requests = Vec::with_capacity(requests_per_thread);
    for i in 0..requests_per_thread {
        let key = format!("key_{}_{}", thread_id, i % 1000);
        requests.push(Request {
            cmd: 1,
            key,
            burst: 100,
            rate: 10,
            period: 60,
            quantity: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as i64,
        });
    }

    // Wait for all threads to be ready
    barrier.wait().await;

    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }

    let mut latencies = Vec::with_capacity(requests_per_thread);
    let mut thread_stats = ThreadLocalStats::default();

    // Send all requests on the same connection
    for request in requests {
        let start = Instant::now();

        // Serialize request
        let mut buf = Vec::new();
        request.serialize(&mut Serializer::new(&mut buf))?;

        match async {
            // Write length prefix and data
            let len = buf.len() as u32;
            stream.write_all(&len.to_be_bytes()).await?;
            stream.write_all(&buf).await?;
            stream.flush().await?;

            // Read response length
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await?;
            let response_len = u32::from_be_bytes(len_buf) as usize;

            // Read response data
            let mut response_buf = vec![0u8; response_len];
            stream.read_exact(&mut response_buf).await?;

            // Deserialize response
            let response: Response =
                Deserialize::deserialize(&mut Deserializer::new(&response_buf[..]))?;

            Ok::<Response, anyhow::Error>(response)
        }
        .await
        {
            Ok(response) => {
                let latency = start.elapsed();
                latencies.push(latency);
                thread_stats.total_requests += 1;
                thread_stats.total_latency_us += latency.as_micros() as u64;

                if response.ok && response.allowed == 1 {
                    thread_stats.successful += 1;
                } else {
                    thread_stats.rate_limited += 1;
                }
            }
            Err(e) => {
                eprintln!("MessagePack protocol error: {e}");
                thread_stats.failed += 1;
                thread_stats.total_requests += 1;
                
                // Try to reconnect
                match TcpStream::connect(format!("127.0.0.1:{port}")).await {
                    Ok(new_stream) => {
                        stream = new_stream;
                        let _ = stream.set_nodelay(true);
                    }
                    Err(_) => break, // Can't reconnect, stop this worker
                }
            }
        }
    }

    Ok((latencies, thread_stats))
}

async fn native_worker(
    thread_id: usize,
    requests_per_thread: usize,
    port: u16,
    barrier: Arc<Barrier>,
    start_flag: Arc<AtomicU64>,
) -> Result<(Vec<Duration>, ThreadLocalStats)> {
    use bytes::{BufMut, BytesMut};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Create a dedicated connection for this worker (no sharing between threads)
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).await?;
    stream.set_nodelay(true)?;

    // Pre-generate key names only (not the full requests with timestamps)
    let mut keys = Vec::with_capacity(requests_per_thread);
    for i in 0..requests_per_thread {
        // Use unique keys to avoid rate limiting during performance tests
        keys.push(format!("key_{thread_id}_req_{i}"));
    }

    // Wait for all threads to be ready
    barrier.wait().await;

    // Wait for start signal
    while start_flag.load(Ordering::Acquire) == 0 {
        tokio::task::yield_now().await;
    }

    let mut latencies = Vec::with_capacity(requests_per_thread);
    let mut thread_stats = ThreadLocalStats::default();
    let mut request_buffer = BytesMut::with_capacity(256);

    // Send all requests on the same connection
    for key in keys {
        let start = Instant::now();

        // Build request with fresh timestamp
        request_buffer.clear();
        request_buffer.put_u8(1); // cmd
        request_buffer.put_u8(key.len() as u8); // key_len
        request_buffer.put_i64_le(100); // burst
        request_buffer.put_i64_le(10); // rate
        request_buffer.put_i64_le(60_000_000_000); // period in nanoseconds
        request_buffer.put_i64_le(1); // quantity

        // Use current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        request_buffer.put_i64_le(now);
        request_buffer.put_slice(key.as_bytes());

        match async {
            // Send request
            stream.write_all(&request_buffer).await?;
            stream.flush().await?;

            // Read response (34 bytes fixed: 1 + 1 + 8*4)
            let mut response = [0u8; 34];
            stream.read_exact(&mut response).await?;

            let ok = response[0];
            let allowed = response[1];

            Ok::<(bool, bool), anyhow::Error>((ok == 1, allowed == 1))
        }
        .await
        {
            Ok((ok, allowed)) => {
                let latency = start.elapsed();
                latencies.push(latency);
                thread_stats.total_requests += 1;
                thread_stats.total_latency_us += latency.as_micros() as u64;

                if ok && allowed {
                    thread_stats.successful += 1;
                } else {
                    thread_stats.rate_limited += 1;
                }
            }
            Err(e) => {
                // Connection failed, try to reconnect for next request
                eprintln!("Native protocol error: {e}");
                thread_stats.failed += 1;
                thread_stats.total_requests += 1;

                // Try to reconnect
                match TcpStream::connect(format!("127.0.0.1:{port}")).await {
                    Ok(new_stream) => {
                        stream = new_stream;
                        let _ = stream.set_nodelay(true);
                    }
                    Err(_) => break, // Can't reconnect, stop this worker
                }
            }
        }
    }

    Ok((latencies, thread_stats))
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
        let barrier = barrier.clone();
        let start_flag = start_flag.clone();
        let transport = transport.clone();

        tasks.spawn(async move {
            match transport {
                Transport::Http => {
                    http_worker(thread_id, requests_per_thread, port, barrier, start_flag).await
                }
                Transport::Grpc => {
                    grpc_worker(thread_id, requests_per_thread, port, barrier, start_flag).await
                }
                Transport::MsgPack => {
                    msgpack_worker(thread_id, requests_per_thread, port, barrier, start_flag).await
                }
                Transport::Native => {
                    native_worker(thread_id, requests_per_thread, port, barrier, start_flag).await
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

    // Collect all latencies and aggregate thread-local stats
    let mut all_latencies = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok((latencies, thread_stats))) => {
                all_latencies.extend(latencies);
                stats.add_thread_stats(&thread_stats);
            }
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
