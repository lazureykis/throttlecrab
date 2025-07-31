use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio::task::JoinSet;

use super::connection_pool::{MsgPackConnectionPool, NativeConnectionPool};
use super::transport_tests::{ServerInstance, Transport};

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub transport: Transport,
    pub store_type: String,
    pub num_threads: usize,
    pub requests_per_thread: usize,
    pub key_pattern: KeyPattern,
    pub total_keys: usize,
}

#[derive(Debug, Clone)]
pub enum KeyPattern {
    Sequential,
    Random,
    Zipfian { alpha: f64 },
}

#[derive(Debug)]
pub struct BenchmarkStats {
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub rate_limited: AtomicU64,
    pub failed_requests: AtomicU64,
    pub total_latency_us: AtomicU64,
}

impl BenchmarkStats {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
        }
    }

    pub fn record_success(&self, latency: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    pub fn record_rate_limited(&self, latency: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn print_summary(&self, duration: Duration) {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful_requests.load(Ordering::Relaxed);
        let rate_limited = self.rate_limited.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);
        let total_latency_us = self.total_latency_us.load(Ordering::Relaxed);

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
    }
}

/// Generate test payloads before starting the benchmark
fn generate_payloads(config: &BenchmarkConfig) -> Vec<Vec<String>> {
    let total_requests = config.num_threads * config.requests_per_thread;
    let mut all_payloads = Vec::with_capacity(total_requests);

    // Generate all payloads
    for i in 0..total_requests {
        let key = match &config.key_pattern {
            KeyPattern::Sequential => format!("key_{i}"),
            KeyPattern::Random => {
                use rand::Rng;
                let key_id = rand::thread_rng().gen_range(0..config.total_keys);
                format!("key_{key_id}")
            }
            KeyPattern::Zipfian { alpha } => {
                let u: f64 = rand::random::<f64>();
                let key_id = ((config.total_keys as f64) * u.powf(-1.0 / alpha)) as usize;
                format!("key_{}", key_id.min(config.total_keys - 1))
            }
        };
        all_payloads.push(key);
    }

    // Split payloads among threads
    let mut thread_payloads = Vec::with_capacity(config.num_threads);
    for i in 0..config.num_threads {
        let start = i * config.requests_per_thread;
        let end = start + config.requests_per_thread;
        thread_payloads.push(all_payloads[start..end].to_vec());
    }

    thread_payloads
}

pub async fn run_high_performance_benchmark(config: BenchmarkConfig) -> Result<()> {
    println!("\n=== High Performance Benchmark ===");
    println!("Transport: {}", config.transport.flag_name());
    println!("Store: {}", config.store_type);
    println!("Threads: {}", config.num_threads);
    println!("Requests per thread: {}", config.requests_per_thread);
    println!(
        "Total requests: {}",
        config.num_threads * config.requests_per_thread
    );

    // Step 1: Start server
    let port = match config.transport {
        Transport::Http => 48080,
        Transport::Grpc => 48070,
        Transport::MsgPack => 48071,
        Transport::Native => 48072,
    };

    let server = ServerInstance::start(config.transport, port, &config.store_type).await?;
    println!("Server started on port {port}");

    // Step 2: Generate payloads before starting threads
    println!(
        "Generating {} payloads...",
        config.num_threads * config.requests_per_thread
    );
    let thread_payloads = generate_payloads(&config);

    // Step 3: Create shared stats and barrier for synchronization
    let stats = Arc::new(BenchmarkStats::new());
    let barrier = Arc::new(Barrier::new(config.num_threads + 1)); // +1 for main thread
    let start_flag = Arc::new(AtomicBool::new(false));

    // Step 4: Create worker threads
    let mut tasks = JoinSet::new();

    for (thread_id, payloads) in thread_payloads.into_iter().enumerate() {
        let stats = stats.clone();
        let barrier = barrier.clone();
        let start_flag = start_flag.clone();
        let transport = config.transport;

        tasks.spawn(async move {
            // Create persistent connection
            let client = match transport {
                Transport::Http => {
                    Box::new(HttpWorkerClient::new(port).await?) as Box<dyn WorkerClient>
                }
                Transport::Grpc => {
                    Box::new(GrpcWorkerClient::new(port).await?) as Box<dyn WorkerClient>
                }
                Transport::MsgPack => {
                    Box::new(MsgPackWorkerClient::new(port).await?) as Box<dyn WorkerClient>
                }
                Transport::Native => {
                    Box::new(NativeWorkerClient::new(port).await?) as Box<dyn WorkerClient>
                }
            };

            // Wait at barrier for all threads to be ready
            barrier.wait().await;

            // Wait for start signal
            while !start_flag.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }

            // Send all requests in sequence
            for key in payloads {
                let start = Instant::now();
                match client.send_request(key).await {
                    Ok(rate_limited) => {
                        let latency = start.elapsed();
                        if rate_limited {
                            stats.record_rate_limited(latency);
                        } else {
                            stats.record_success(latency);
                        }
                    }
                    Err(_) => {
                        stats.record_failure();
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        });
    }

    // Wait for all threads to be ready
    println!("Waiting for all threads to establish connections...");
    barrier.wait().await;

    // Start the benchmark
    println!("Starting benchmark...");
    let bench_start = Instant::now();
    start_flag.store(true, Ordering::Release);

    // Wait for all threads to complete
    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            eprintln!("Thread error: {e}");
        }
    }

    let duration = bench_start.elapsed();

    // Print results
    stats.print_summary(duration);

    // Stop server
    server.stop().await?;

    Ok(())
}

// Worker client trait for different transports
#[async_trait::async_trait]
trait WorkerClient: Send + Sync {
    async fn send_request(&self, key: String) -> Result<bool>;
}

// HTTP worker client with persistent connection
struct HttpWorkerClient {
    client: reqwest::Client,
    url: String,
}

impl HttpWorkerClient {
    async fn new(port: u16) -> Result<Self> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(1)
            .build()?;

        Ok(Self {
            client,
            url: format!("http://127.0.0.1:{port}/throttle"),
        })
    }
}

#[async_trait::async_trait]
impl WorkerClient for HttpWorkerClient {
    async fn send_request(&self, key: String) -> Result<bool> {
        let response = self
            .client
            .post(&self.url)
            .json(&serde_json::json!({
                "key": key,
                "max_burst": 100,
                "count_per_period": 10,
                "period": 60,
                "quantity": 1,
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        Ok(!json["allowed"].as_bool().unwrap_or(true))
    }
}

// gRPC worker client
struct GrpcWorkerClient {
    client: throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient<
        tonic::transport::Channel,
    >,
}

impl GrpcWorkerClient {
    async fn new(port: u16) -> Result<Self> {
        use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;

        let client = RateLimiterClient::connect(format!("http://127.0.0.1:{port}")).await?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl WorkerClient for GrpcWorkerClient {
    async fn send_request(&self, key: String) -> Result<bool> {
        use throttlecrab_server::grpc::ThrottleRequest;

        let mut client = self.client.clone();
        let request = tonic::Request::new(ThrottleRequest {
            key,
            max_burst: 100,
            count_per_period: 10,
            period: 60,
            quantity: 1,
            timestamp: 0,
        });

        let response = client.throttle(request).await?;
        Ok(!response.into_inner().allowed)
    }
}

// MessagePack worker client
struct MsgPackWorkerClient {
    pool: MsgPackConnectionPool,
}

impl MsgPackWorkerClient {
    async fn new(port: u16) -> Result<Self> {
        // Use a smaller pool for each worker to avoid contention
        let pool = MsgPackConnectionPool::new(port, 2);
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl WorkerClient for MsgPackWorkerClient {
    async fn send_request(&self, key: String) -> Result<bool> {
        self.pool.test_request(key).await
    }
}

// Native protocol worker client
struct NativeWorkerClient {
    pool: NativeConnectionPool,
}

impl NativeWorkerClient {
    async fn new(port: u16) -> Result<Self> {
        // Use a smaller pool for each worker to avoid contention
        let pool = NativeConnectionPool::new(port, 2);
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl WorkerClient for NativeWorkerClient {
    async fn send_request(&self, key: String) -> Result<bool> {
        self.pool.test_request(key).await
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_high_performance_http() -> Result<()> {
        let config = BenchmarkConfig {
            transport: Transport::Http,
            store_type: "periodic".to_string(),
            num_threads: 10,
            requests_per_thread: 1000,
            key_pattern: KeyPattern::Random,
            total_keys: 1000,
        };

        run_high_performance_benchmark(config).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_high_performance_native() -> Result<()> {
        let config = BenchmarkConfig {
            transport: Transport::Native,
            store_type: "adaptive".to_string(),
            num_threads: 20,
            requests_per_thread: 5000,
            key_pattern: KeyPattern::Zipfian { alpha: 1.2 },
            total_keys: 10000,
        };

        run_high_performance_benchmark(config).await?;
        Ok(())
    }
}
