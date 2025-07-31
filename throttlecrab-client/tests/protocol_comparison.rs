use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use throttlecrab_client::ClientBuilder;
use throttlecrab_server::actor::RateLimiterActor;
use throttlecrab_server::transport::{
    Transport, msgpack::MsgPackTransport, native::NativeTransport,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

struct ProtocolStats {
    name: String,
    total_requests: AtomicU64,
    total_latency_us: AtomicU64,
}

impl ProtocolStats {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            total_requests: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
        }
    }

    fn record(&self, latency: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us
            .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    }

    fn print_summary(&self, duration: Duration) {
        let total = self.total_requests.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);

        let rps = total as f64 / duration.as_secs_f64();
        let avg_latency = if total > 0 { total_latency / total } else { 0 };

        println!("{} Protocol:", self.name);
        println!("  Total requests: {total}");
        println!("  Throughput: {rps:.2} req/sec");
        println!("  Average latency: {avg_latency} μs");
    }
}

async fn setup_servers() -> (u16, u16) {
    // Start native protocol server
    let store1 = throttlecrab::PeriodicStore::builder()
        .capacity(1_000_000)
        .cleanup_interval(Duration::from_secs(300))
        .build();
    let limiter1 = RateLimiterActor::spawn_periodic(1_000_000, store1);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let native_port = listener.local_addr().unwrap().port();
    drop(listener);

    let native_transport = NativeTransport::new("127.0.0.1", native_port);
    tokio::spawn(async move {
        native_transport.start(limiter1).await.unwrap();
    });

    // Start MessagePack protocol server
    let store2 = throttlecrab::PeriodicStore::builder()
        .capacity(1_000_000)
        .cleanup_interval(Duration::from_secs(300))
        .build();
    let limiter2 = RateLimiterActor::spawn_periodic(1_000_000, store2);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let msgpack_port = listener.local_addr().unwrap().port();
    drop(listener);

    let msgpack_transport = MsgPackTransport::new("127.0.0.1", msgpack_port);
    tokio::spawn(async move {
        msgpack_transport.start(limiter2).await.unwrap();
    });

    sleep(Duration::from_millis(200)).await;

    (native_port, msgpack_port)
}

async fn msgpack_client_request(
    stream: &mut TcpStream,
    key: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
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

    let request = Request {
        cmd: 1,
        key: key.to_string(),
        burst: 1000,
        rate: 10000,
        period: 60,
        quantity: 1,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
    };

    // Serialize request
    let mut buf = Vec::new();
    request.serialize(&mut Serializer::new(&mut buf))?;

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
    let response: Response = Deserialize::deserialize(&mut Deserializer::new(&response_buf[..]))?;

    Ok(response.ok && response.allowed == 1)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_protocol_performance_comparison() {
    let (native_port, msgpack_port) = setup_servers().await;

    println!("=== Protocol Performance Comparison ===");
    println!("Test duration: 10 seconds");
    println!("Concurrent workers: 5\n");

    let test_duration = Duration::from_secs(10);
    let num_workers = 5;

    // Test native protocol
    println!("Testing Native Protocol...");
    let native_client = ClientBuilder::new()
        .max_connections(10)
        .min_idle_connections(5)
        .tcp_nodelay(true)
        .build(format!("127.0.0.1:{native_port}"))
        .await
        .unwrap();

    let native_stats = Arc::new(ProtocolStats::new("Native"));
    let start_time = Instant::now();

    let mut handles = vec![];
    for worker_id in 0..num_workers {
        let client = native_client.clone();
        let stats = native_stats.clone();
        handles.push(tokio::spawn(async move {
            let mut count = 0;
            while start_time.elapsed() < test_duration {
                let key = format!("native_worker_{worker_id}_req_{count}");
                let req_start = Instant::now();

                if let Ok(_response) = client.check_rate_limit(&key, 1000, 10000, 60).await {
                    stats.record(req_start.elapsed());
                }
                count += 1;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let native_duration = start_time.elapsed();

    // Test MessagePack protocol
    println!("\nTesting MessagePack Protocol...");
    let msgpack_stats = Arc::new(ProtocolStats::new("MessagePack"));
    let start_time = Instant::now();

    let mut handles = vec![];
    for worker_id in 0..num_workers {
        let stats = msgpack_stats.clone();
        let port = msgpack_port;
        handles.push(tokio::spawn(async move {
            // Create dedicated connection for this worker
            let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .unwrap();
            stream.set_nodelay(true).unwrap();

            let mut count = 0;
            while start_time.elapsed() < test_duration {
                let key = format!("msgpack_worker_{worker_id}_req_{count}");
                let req_start = Instant::now();

                if let Ok(_allowed) = msgpack_client_request(&mut stream, &key).await {
                    stats.record(req_start.elapsed());
                }
                count += 1;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let msgpack_duration = start_time.elapsed();

    // Print comparison
    println!("\n=== Results ===");
    native_stats.print_summary(native_duration);
    println!();
    msgpack_stats.print_summary(msgpack_duration);

    // Calculate performance difference
    let native_rps =
        native_stats.total_requests.load(Ordering::Relaxed) as f64 / native_duration.as_secs_f64();
    let msgpack_rps = msgpack_stats.total_requests.load(Ordering::Relaxed) as f64
        / msgpack_duration.as_secs_f64();
    let speedup = native_rps / msgpack_rps;

    println!("\n=== Performance Comparison ===");
    println!("Native protocol is {speedup:.2}x faster than MessagePack");

    let native_avg_latency = native_stats.total_latency_us.load(Ordering::Relaxed)
        / native_stats.total_requests.load(Ordering::Relaxed);
    let msgpack_avg_latency = msgpack_stats.total_latency_us.load(Ordering::Relaxed)
        / msgpack_stats.total_requests.load(Ordering::Relaxed);

    println!(
        "Native average latency: {native_avg_latency} μs vs MessagePack: {msgpack_avg_latency} μs ({:.2}x improvement)",
        msgpack_avg_latency as f64 / native_avg_latency as f64
    );

    // Assert native is faster
    assert!(
        speedup > 1.0,
        "Native protocol should be faster than MessagePack"
    );
}
